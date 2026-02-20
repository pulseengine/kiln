//! Simple working WebAssembly execution engine
//!
//! This module implements a basic stackless WebAssembly execution engine
//! focused on functionality over advanced features. It provides the interface
//! needed by CapabilityAwareEngine to execute WASM modules.

// Import tracing utilities
#[cfg(feature = "tracing")]
use wrt_foundation::tracing::{
    ExecutionTrace, ImportTrace, ModuleTrace, MemoryTrace,
    debug, trace, info, warn, error,
    debug_span, info_span, trace_span, Span
};

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{
    collections::BTreeMap as HashMap,
    string::String,
    sync::Arc,
    vec::Vec,
};
use core::sync::atomic::{
    AtomicU64,
    Ordering,
};
// Use std types when available, fall back to alloc, then wrt_foundation
#[cfg(feature = "std")]
use std::{
    collections::HashMap,
    string::String,
    sync::Arc,
    vec::Vec,
};

// Debug support - only available with std and wrt-debug crate
#[cfg(all(feature = "std", feature = "debugger"))]
use wrt_debug::runtime_traits::{RuntimeDebugger, RuntimeState, DebugAction};

// For pure no_std without alloc, use bounded collections
#[cfg(not(any(feature = "std", feature = "alloc")))]
use wrt_foundation::{
    bounded::BoundedString,
    bounded::BoundedVec,
    bounded_collections::BoundedMap,
    safe_memory::NoStdProvider,
};

// Type aliases for pure no_std mode
#[cfg(not(any(feature = "std", feature = "alloc")))]
type HashMap<K, V> = BoundedMap<K, V, 16, NoStdProvider<4096>>; // 16 concurrent instances max
#[cfg(not(any(feature = "std", feature = "alloc")))]
type Vec<T> = BoundedVec<T, 256, NoStdProvider<4096>>; // 256 operands max
#[cfg(not(any(feature = "std", feature = "alloc")))]
type String = BoundedString<256>; // 256 byte strings

// Simple Arc substitute for no_std - just owns the value directly
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub struct Arc<T>(T);

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T> Arc<T> {
    pub fn new(value: T) -> Self {
        Arc(value)
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T> core::ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T: Clone> Clone for Arc<T> {
    fn clone(&self) -> Self {
        Arc(self.0.clone())
    }
}

// Implement required traits for Arc to work with bounded collections
#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T> wrt_foundation::traits::Checksummable for Arc<T>
where
    T: wrt_foundation::traits::Checksummable,
{
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T> wrt_foundation::traits::ToBytes for Arc<T>
where
    T: wrt_foundation::traits::ToBytes,
{
    fn serialized_size(&self) -> usize {
        self.0.serialized_size()
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T> wrt_foundation::traits::FromBytes for Arc<T>
where
    T: wrt_foundation::traits::FromBytes,
{
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let value = T::from_bytes_with_provider(reader, provider)?;
        Ok(Arc::new(value))
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T: Default> Default for Arc<T> {
    fn default() -> Self {
        Arc::new(T::default())
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T: PartialEq> PartialEq for Arc<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<T: Eq> Eq for Arc<T> {}

use wrt_error::Result;
use wrt_foundation::{
    traits::BoundedCapacity,
    values::{
        FloatBits32,
        FloatBits64,
        Value,
    },
};

use crate::module_instance::ModuleInstance;

/// Strip the version suffix from a WASI interface name.
/// e.g., "wasi:cli/stdout@0.2.4" -> "wasi:cli/stdout"
/// This allows matching any 0.2.x version against our implementations.
fn strip_wasi_version(interface: &str) -> &str {
    if let Some(at_pos) = interface.find('@') {
        &interface[..at_pos]
    } else {
        interface
    }
}

/// Check if two function types match structurally.
/// WebAssembly uses structural type equivalence: two function types are compatible
/// if they have the same parameter and result types in the same order.
fn func_types_match(expected: &wrt_foundation::types::FuncType, actual: &wrt_foundation::types::FuncType) -> bool {
    if expected.params.len() != actual.params.len() {
        return false;
    }
    if expected.results.len() != actual.results.len() {
        return false;
    }
    // Compare each parameter type
    for i in 0..expected.params.len() {
        let expected_param = expected.params.get(i);
        let actual_param = actual.params.get(i);
        match (expected_param, actual_param) {
            (Some(e), Some(a)) if e == a => continue,
            _ => return false,
        }
    }
    // Compare each result type
    for i in 0..expected.results.len() {
        let expected_result = expected.results.get(i);
        let actual_result = actual.results.get(i);
        match (expected_result, actual_result) {
            (Some(e), Some(a)) if e == a => continue,
            _ => return false,
        }
    }
    true
}

/// Maximum number of concurrent module instances
/// Set to 512 to handle large WAST test files that may have hundreds of module directives
/// (e.g., align.wast has 117 module directives including assert_invalid)
const MAX_CONCURRENT_INSTANCES: usize = 512;

/// Maximum call depth to prevent infinite recursion.
/// With trampolining, wasm-to-wasm calls don't grow the Rust stack,
/// so this can be set high. Only cross-instance calls via call_exported_function
/// create nested trampolines (bounded by import chain depth).
const MAX_CALL_DEPTH: usize = 10000;

/// Saved execution state for a suspended function during a Call.
/// When a Call instruction is encountered, the caller's state is saved here
/// and returned to the trampoline. When the callee completes, the trampoline
/// pushes results onto operand_stack and resumes execution from pc.
#[cfg(any(feature = "std", feature = "alloc"))]
#[derive(Debug)]
struct SuspendedFrame {
    /// Instance ID of the executing instance
    instance_id: usize,
    /// Function index being executed (used to re-lookup module/instructions on resume)
    func_idx: usize,
    /// Program counter to resume from (instruction AFTER the Call/CallIndirect)
    pc: usize,
    /// Local variables (moved from execute_function_body, not cloned)
    locals: Vec<Value>,
    /// Operand stack (moved, callee results will be pushed here before resume)
    operand_stack: Vec<Value>,
    /// Block stack: (block_type, start_pc, block_type_idx, entry_stack_height)
    block_stack: Vec<(&'static str, usize, u32, usize)>,
    /// Block depth counter
    block_depth: i32,
    /// Instruction count for profiling
    instruction_count: usize,
}

/// Simple execution statistics
#[derive(Debug, Default)]
pub struct ExecutionStats {
    /// Number of function calls executed
    pub function_calls: u64,
}

/// Outcome of executing a function body.
/// Used for trampolining: instead of recursing on the Rust stack,
/// execute_function_body returns control flow decisions to the trampoline in execute().
#[cfg(any(feature = "std", feature = "alloc"))]
enum ExecutionOutcome {
    /// Function completed normally with results
    Complete(Vec<Value>),
    /// Tail call to another function - trampoline should continue with new target
    TailCall {
        /// Target function index
        func_idx: usize,
        /// Arguments for the target function
        args: Vec<Value>,
    },
    /// Regular call to another function - save caller state and execute callee
    Call {
        /// Target instance ID for the callee
        instance_id: usize,
        /// Target function index for the callee
        func_idx: usize,
        /// Arguments for the callee
        args: Vec<Value>,
        /// Caller's saved state. Some = regular call (push onto pending stack).
        /// None = import redirect (no state to save, caller already on pending stack).
        return_state: Option<SuspendedFrame>,
    },
}

/// Pre-allocated WASI stub memory regions
#[derive(Debug, Clone)]
pub struct WasiStubMemory {
    /// Pointer to empty list structure (ptr=0, len=0)
    pub empty_list: u32,
    /// Pointer to option None discriminant (0)
    pub option_none: u32,
    /// Pointer to empty environment list
    pub empty_env: u32,
    /// Stdout handle value
    pub stdout_handle: u32,
    /// Stderr handle value
    pub stderr_handle: u32,
}

/// A lowered function produced by canon.lower
///
/// Per Component Model spec, `canon lower` takes a component function and produces
/// a core WebAssembly function. This struct represents that synthesized function.
/// When the engine executes a function that maps to a LoweredFunction, it dispatches
/// to the canonical executor instead of executing bytecode.
///
/// This is the "virtual WebAssembly code" approach - the lowered function exists
/// in the function index space but executes via our canonical executor.
#[derive(Debug, Clone)]
#[cfg(feature = "std")]
pub struct LoweredFunction {
    /// Target interface (e.g., "wasi:io/streams@0.2.4")
    pub interface: String,
    /// Target function name (e.g., "[method]output-stream.blocking-write-and-flush")
    pub function: String,
    /// Memory index for ABI operations (from canon options)
    pub memory_idx: Option<u32>,
    /// Realloc function index for ABI operations (from canon options)
    pub realloc_idx: Option<u32>,
}

/// Simple stackless WebAssembly execution engine
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct StacklessEngine {
    /// Currently loaded instances indexed by numeric ID
    instances:             HashMap<usize, Arc<ModuleInstance>>,
    /// Next instance ID
    next_instance_id:      AtomicU64,
    /// Current active instance for execution
    current_instance_id:   Option<usize>,
    /// Operand stack for execution (needed by tail_call module)
    pub operand_stack:     Vec<Value>,
    /// Call frames count (needed by tail_call module)
    pub call_frames_count: usize,
    /// Execution statistics (needed by tail_call module)
    pub stats:             ExecutionStats,
    /// Remaining fuel for execution
    fuel:                  AtomicU64,
    /// Current instruction pointer
    instruction_pointer:   AtomicU64,
    /// Host function registry for calling imported functions
    #[cfg(feature = "std")]
    host_registry:         Option<Arc<wrt_host::CallbackRegistry>>,
    /// Pre-allocated WASI stub memory for each instance
    wasi_stubs:            HashMap<usize, WasiStubMemory>,
    /// Dropped data segments per instance (for data.drop/memory.init)
    /// Maps instance_id -> Vec<bool> where true means segment was dropped
    dropped_data_segments: HashMap<usize, Vec<bool>>,
    /// Cross-instance import links: (instance_id, import_module, import_name) -> (target_instance_id, export_name)
    #[cfg(feature = "std")]
    import_links:          HashMap<(usize, String, String), (usize, String)>,
    /// Aliased function origins: (instance_id, func_idx) -> original_instance_id
    /// Tracks which instance an aliased function actually comes from
    #[cfg(feature = "std")]
    aliased_functions:     HashMap<(usize, usize), usize>,
    /// WASI dispatcher for proper WASI host function implementations
    #[cfg(feature = "wasi")]
    wasi_dispatcher:       Option<wrt_wasi::WasiDispatcher>,
    /// Generic host import handler for all host function calls
    #[cfg(feature = "std")]
    host_handler:          Option<Box<dyn wrt_foundation::HostImportHandler>>,
    /// Optional runtime debugger for profiling and debugging
    #[cfg(all(feature = "std", feature = "debugger"))]
    debugger:              Option<Box<dyn RuntimeDebugger>>,
    /// Active exception state for exception propagation across calls
    /// Contains (instance_id, tag_idx, tag_identity, payload) when an exception is in flight
    /// tag_identity is Some((module, name)) for imported tags, None for local tags
    #[cfg(feature = "std")]
    active_exception:      Option<(usize, u32, Option<(String, String)>, Vec<Value>)>,
    /// Storage for caught exceptions (for throw_ref to re-throw)
    /// Maps exnref index to (tag_idx, payload)
    #[cfg(feature = "std")]
    /// Exception storage: (tag_idx, tag_identity, payload)
    /// tag_identity is Some((module, name)) for imported tags, None for local tags
    exception_storage:     Vec<(u32, Option<(String, String)>, Vec<Value>)>,
    /// Lowered function registry: (instance_id, func_idx) -> LoweredFunction
    /// Used for canon.lower synthesized functions that dispatch to canonical executor
    #[cfg(feature = "std")]
    lowered_functions:     HashMap<(usize, usize), LoweredFunction>,
    /// Instance name registry: instance_id -> registered module name
    /// Used for resolving tag identities in cross-module exception handling
    #[cfg(feature = "std")]
    instance_registry:     HashMap<usize, String>,
    /// Explicit call stack for trampolining (avoids Rust stack recursion)
    /// Not used directly as a field - the trampoline in execute() uses a local Vec instead.
    /// Kept for potential future use (e.g., stack inspection).
    #[cfg(feature = "std")]
    call_stack:            Vec<SuspendedFrame>,
}

/// Simple stackless WebAssembly execution engine (no_std version)
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub struct StacklessEngine {
    /// Currently loaded instances indexed by numeric ID
    instances:             HashMap<usize, Arc<ModuleInstance>>,
    /// Next instance ID
    next_instance_id:      AtomicU64,
    /// Current active instance for execution
    current_instance_id:   Option<usize>,
    /// Operand stack for execution (needed by tail_call module)
    pub operand_stack:     Vec<Value>,
    /// Call frames count (needed by tail_call module)
    pub call_frames_count: usize,
    /// Execution statistics (needed by tail_call module)
    pub stats:             ExecutionStats,
    /// Remaining fuel for execution
    fuel:                  AtomicU64,
    /// Current instruction pointer
    instruction_pointer:   AtomicU64,
}

/// Simple RuntimeState implementation for debugger callbacks
#[cfg(all(feature = "std", feature = "debugger"))]
pub struct ExecutionState<'a> {
    pc: u32,
    func_idx: u32,
    operand_stack: &'a [Value],
    locals: &'a [Value],
}

#[cfg(all(feature = "std", feature = "debugger"))]
impl<'a> RuntimeState for ExecutionState<'a> {
    fn pc(&self) -> u32 {
        self.pc
    }

    fn sp(&self) -> u32 {
        self.operand_stack.len() as u32
    }

    fn fp(&self) -> Option<u32> {
        Some(0) // WebAssembly doesn't have a traditional frame pointer
    }

    fn read_local(&self, index: u32) -> Option<u64> {
        self.locals.get(index as usize).map(|v| match v {
            Value::I32(x) => *x as u64,
            Value::I64(x) => *x as u64,
            Value::F32(x) => x.to_bits() as u64,
            Value::F64(x) => x.to_bits(),
            _ => 0,
        })
    }

    fn read_stack(&self, offset: u32) -> Option<u64> {
        let len = self.operand_stack.len();
        if offset as usize >= len {
            return None;
        }
        let idx = len - 1 - offset as usize;
        self.operand_stack.get(idx).map(|v| match v {
            Value::I32(x) => *x as u64,
            Value::I64(x) => *x as u64,
            Value::F32(x) => x.to_bits() as u64,
            Value::F64(x) => x.to_bits(),
            _ => 0,
        })
    }

    fn current_function(&self) -> Option<u32> {
        Some(self.func_idx)
    }
}

/// Calculate effective memory address with overflow checking.
/// Per WebAssembly spec, if base + offset overflows or exceeds u32::MAX, it traps.
/// Returns Ok(effective_address) or Err if overflow occurs.
#[inline]
fn calculate_effective_address(base: i32, offset: u32, size: u32) -> wrt_error::Result<u64> {
    // Convert base to u32 first (WebAssembly treats addresses as unsigned)
    let base_u32 = base as u32;

    // Check for overflow in base + offset
    let effective_addr = (base_u32 as u64)
        .checked_add(offset as u64)
        .ok_or_else(|| wrt_error::Error::runtime_trap("out of bounds memory access"))?;

    // Check for overflow when adding access size
    let end_addr = effective_addr
        .checked_add(size as u64)
        .ok_or_else(|| wrt_error::Error::runtime_trap("out of bounds memory access"))?;

    // If end_addr exceeds u32::MAX + 1 (4GB), it's out of bounds for 32-bit memory
    // But we let the actual memory bounds check handle the memory size comparison
    // Just ensure the calculation doesn't overflow
    if end_addr > u64::from(u32::MAX) + 1 {
        return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
    }

    Ok(effective_addr)
}

impl StacklessEngine {
    /// Create a new stackless engine
    #[cfg(any(feature = "std", feature = "alloc"))]
    pub fn new() -> Self {
        Self {
            instances:           HashMap::new(),
            next_instance_id:    AtomicU64::new(1),
            current_instance_id: None,
            operand_stack:       Vec::new(),
            call_frames_count:   0,
            stats:               ExecutionStats::default(),
            fuel:                AtomicU64::new(u64::MAX),
            instruction_pointer: AtomicU64::new(0),
            #[cfg(feature = "std")]
            host_registry:       None,
            wasi_stubs:          HashMap::new(),
            dropped_data_segments: HashMap::new(),
            #[cfg(feature = "std")]
            import_links:        HashMap::new(),
            #[cfg(feature = "std")]
            aliased_functions:   HashMap::new(),
            #[cfg(feature = "wasi")]
            wasi_dispatcher:     wrt_wasi::WasiDispatcher::with_defaults().ok(),
            #[cfg(feature = "std")]
            host_handler:        None,
            #[cfg(all(feature = "std", feature = "debugger"))]
            debugger:            None,
            #[cfg(feature = "std")]
            active_exception:    None,
            #[cfg(feature = "std")]
            exception_storage:   Vec::new(),
            #[cfg(feature = "std")]
            lowered_functions:   HashMap::new(),
            #[cfg(feature = "std")]
            instance_registry:   HashMap::new(),
            #[cfg(feature = "std")]
            call_stack:          Vec::with_capacity(256),
        }
    }

    /// Set a runtime debugger for profiling and debugging
    ///
    /// When a debugger is attached, it will receive callbacks for each instruction,
    /// function entry/exit, and traps. This is useful for profiling and debugging.
    ///
    /// Note: This has performance overhead - only use when needed.
    #[cfg(all(feature = "std", feature = "debugger"))]
    pub fn set_debugger(&mut self, debugger: Box<dyn RuntimeDebugger>) {
        self.debugger = Some(debugger);
    }

    /// Remove the attached debugger
    #[cfg(all(feature = "std", feature = "debugger"))]
    pub fn clear_debugger(&mut self) {
        self.debugger = None;
    }

    /// Check if a debugger is attached
    #[cfg(all(feature = "std", feature = "debugger"))]
    pub fn has_debugger(&self) -> bool {
        self.debugger.is_some()
    }

    /// Set the host import handler for resolving host function calls
    #[cfg(feature = "std")]
    pub fn set_host_handler(&mut self, handler: Box<dyn wrt_foundation::HostImportHandler>) {
        self.host_handler = Some(handler);
    }

    /// Set WASI command-line arguments
    ///
    /// These arguments will be returned by `wasi:cli/environment::get-arguments`
    #[cfg(feature = "wasi")]
    pub fn set_wasi_args(&mut self, args: Vec<String>) {
        if let Some(ref mut dispatcher) = self.wasi_dispatcher {
            dispatcher.set_args(args);
        }
    }

    /// Set WASI environment variables
    ///
    /// These will be returned by `wasi:cli/environment::get-environment`
    #[cfg(feature = "wasi")]
    pub fn set_wasi_env(&mut self, env_vars: Vec<(String, String)>) {
        if let Some(ref mut dispatcher) = self.wasi_dispatcher {
            dispatcher.set_env(env_vars);
        }
    }

    /// Register a lowered function from a canon.lower operation
    ///
    /// When a module calls a function at this (instance_id, func_idx), the engine
    /// will dispatch to the canonical executor instead of executing bytecode.
    /// This is the Component Model's "synthesized core function" mechanism.
    #[cfg(feature = "std")]
    pub fn register_lowered_function(
        &mut self,
        instance_id: usize,
        func_idx: usize,
        interface: String,
        function: String,
        memory_idx: Option<u32>,
        realloc_idx: Option<u32>,
    ) {
        #[cfg(feature = "tracing")]
        trace!(
            instance_id = instance_id,
            func_idx = func_idx,
            interface = %interface,
            function = %function,
            "[CANON_LOWER] Registering lowered function"
        );
        let lowered = LoweredFunction {
            interface,
            function,
            memory_idx,
            realloc_idx,
        };
        self.lowered_functions.insert((instance_id, func_idx), lowered);
    }

    /// Register an instance name for cross-module exception handling
    ///
    /// This associates a human-readable module name with an instance ID.
    /// Used for resolving tag identities in catch handlers when exceptions
    /// propagate across module boundaries.
    #[cfg(feature = "std")]
    pub fn register_instance_name(&mut self, instance_id: usize, name: &str) {
        self.instance_registry.insert(instance_id, name.to_string());
    }

    /// Get the registered name for an instance
    #[cfg(feature = "std")]
    pub fn get_instance_registered_name(&self, instance_id: usize) -> Option<&String> {
        self.instance_registry.get(&instance_id)
    }

    /// Get the effective tag identity for exception matching.
    ///
    /// This function returns an identity that can be used to match exceptions across modules:
    /// - For imported tags: returns the import source (module_name, field_name)
    /// - For exported local tags (with registered instance): returns (registered_name, export_name)
    /// - For non-exported local tags: returns None (only matches within same module)
    #[cfg(feature = "std")]
    fn get_effective_tag_identity(
        &self,
        instance_id: usize,
        module: &crate::module::Module,
        tag_idx: u32,
    ) -> Option<(String, String)> {
        // First check if the tag is imported
        if let Some(import_identity) = module.get_tag_import_identity(tag_idx) {
            return Some(import_identity);
        }

        // Tag is local - check if it's exported and instance is registered
        if let Some(export_name) = module.get_tag_export_name(tag_idx) {
            if let Some(registered_name) = self.get_instance_registered_name(instance_id) {
                return Some((registered_name.clone(), export_name));
            }
        }

        // Local tag without export or unregistered instance - no cross-module identity
        None
    }

    /// Check if a function is a lowered function (from canon.lower)
    #[cfg(feature = "std")]
    fn is_lowered_function(&self, instance_id: usize, func_idx: usize) -> bool {
        self.lowered_functions.contains_key(&(instance_id, func_idx))
    }

    /// Execute a lowered function via the WASI dispatcher
    ///
    /// This is called when the engine detects that a function is a lowered function.
    /// It extracts the target interface/function and dispatches to the WASI system.
    ///
    /// Implements canonical ABI lifting: converts core WASM values (i32 pointers/lengths)
    /// to component values (lists, records, etc.) by reading from instance memory.
    #[cfg(all(feature = "std", feature = "wasi"))]
    fn execute_lowered_function(
        &mut self,
        instance_id: usize,
        func_idx: usize,
        args: Vec<Value>,
    ) -> Result<Vec<Value>> {
        let lowered = self.lowered_functions.get(&(instance_id, func_idx))
            .cloned()
            .ok_or_else(|| wrt_error::Error::runtime_error("Lowered function not found"))?;

        #[cfg(feature = "tracing")]
        trace!(
            interface = %lowered.interface,
            function = %lowered.function,
            args_count = args.len(),
            "[CANON_LOWER] Executing lowered function"
        );

        // Lift args based on the function being called
        // Some functions need memory access to convert pointers to actual data
        let wasi_args = self.lift_lowered_function_args(
            instance_id,
            &lowered.interface,
            &lowered.function,
            &args,
        )?;

        // Dispatch to WASI using the standard dispatch interface
        if let Some(ref mut dispatcher) = self.wasi_dispatcher {
            let wasi_results = dispatcher.dispatch(&lowered.interface, &lowered.function, &wasi_args)?;

            // Convert wrt_wasi::Value back to wrt_foundation::values::Value
            let results: Vec<Value> = wasi_results.into_iter().map(|v| {
                match v {
                    wrt_wasi::Value::S32(i) => Value::I32(i),
                    wrt_wasi::Value::U32(u) => Value::I32(u as i32),
                    wrt_wasi::Value::S64(i) => Value::I64(i),
                    wrt_wasi::Value::U64(u) => Value::I64(u as i64),
                    wrt_wasi::Value::F32(f) => Value::F32(FloatBits32::from_f32(f)),
                    wrt_wasi::Value::F64(f) => Value::F64(FloatBits64::from_f64(f)),
                    wrt_wasi::Value::Bool(b) => Value::I32(if b { 1 } else { 0 }),
                    wrt_wasi::Value::U8(u) => Value::I32(u as i32),
                    wrt_wasi::Value::S8(i) => Value::I32(i as i32),
                    wrt_wasi::Value::U16(u) => Value::I32(u as i32),
                    wrt_wasi::Value::S16(i) => Value::I32(i as i32),
                    _ => Value::I32(0), // Default for unsupported types
                }
            }).collect();

            Ok(results)
        } else {
            Err(wrt_error::Error::runtime_error("WASI dispatcher not available for lowered function"))
        }
    }

    /// Lift core WASM args to component-level WASI args
    ///
    /// This implements the canonical ABI "lift" operation for function arguments.
    /// For functions that take list<u8> parameters (like write operations), we need
    /// to read the actual bytes from memory using the pointer and length.
    #[cfg(all(feature = "std", feature = "wasi"))]
    fn lift_lowered_function_args(
        &self,
        instance_id: usize,
        interface: &str,
        function: &str,
        args: &[Value],
    ) -> Result<Vec<wrt_wasi::Value>> {
        // Check if this function needs special ABI lifting
        let needs_write_lifting = function.contains("blocking-write")
            || function.contains("write-zeroes")
            || function == "write";

        if needs_write_lifting && args.len() >= 3 {
            // blocking-write-and-flush ABI: (handle: u32, data_ptr: i32, data_len: i32, retptr: i32)
            // We need to read the bytes from memory at data_ptr for data_len bytes
            return self.lift_write_args(instance_id, args);
        }

        // For other functions, do simple value conversion
        let wasi_args: Vec<wrt_wasi::Value> = args.iter().map(|v| {
            match v {
                Value::I32(i) => wrt_wasi::Value::S32(*i),
                Value::I64(i) => wrt_wasi::Value::S64(*i),
                Value::F32(f) => wrt_wasi::Value::F32(f.to_f32()),
                Value::F64(f) => wrt_wasi::Value::F64(f.to_f64()),
                _ => wrt_wasi::Value::U32(0),
            }
        }).collect();

        Ok(wasi_args)
    }

    /// Lift write operation arguments by reading data from memory
    ///
    /// For blocking-write-and-flush: (handle, data_ptr, data_len, retptr)
    /// Returns: [handle, List<U8>(actual bytes)]
    #[cfg(all(feature = "std", feature = "wasi"))]
    fn lift_write_args(
        &self,
        instance_id: usize,
        args: &[Value],
    ) -> Result<Vec<wrt_wasi::Value>> {
        // Extract the pointer and length from args
        let handle = match args.get(0) {
            Some(Value::I32(h)) => *h as u32,
            _ => return Err(wrt_error::Error::runtime_error("Missing handle argument for write")),
        };

        let data_ptr = match args.get(1) {
            Some(Value::I32(p)) => *p as u32,
            _ => return Err(wrt_error::Error::runtime_error("Missing data pointer for write")),
        };

        let data_len = match args.get(2) {
            Some(Value::I32(l)) => *l as u32,
            _ => return Err(wrt_error::Error::runtime_error("Missing data length for write")),
        };

        #[cfg(feature = "tracing")]
        trace!(
            handle = handle,
            data_ptr = data_ptr,
            data_len = data_len,
            instance_id = instance_id,
            "[CANON_LIFT] Lifting write args from memory"
        );

        // Find an instance with memory - the shim doesn't have memory, but the main module does.
        // In Component Model, the memory comes from the module that imports the lowered function,
        // not the shim. We search for an instance that has memory.
        let memory = self.find_instance_with_memory(instance_id)?;

        let mut data = Vec::with_capacity(data_len as usize);
        for i in 0..data_len {
            let addr = data_ptr + i;
            let mut buffer = [0u8; 1];
            memory.0.read(addr, &mut buffer)
                .map_err(|_| wrt_error::Error::runtime_error("Failed to read memory for write data"))?;
            data.push(buffer[0]);
        }

        #[cfg(feature = "tracing")]
        trace!(
            bytes_read = data.len(),
            "[CANON_LIFT] Read {} bytes from memory",
            data.len()
        );

        // Convert to WASI values: handle and list of bytes
        let byte_list: Vec<wrt_wasi::Value> = data.into_iter()
            .map(wrt_wasi::Value::U8)
            .collect();

        Ok(vec![
            wrt_wasi::Value::U32(handle),
            wrt_wasi::Value::List(byte_list),
        ])
    }

    /// Find an instance that has memory for ABI lifting
    ///
    /// In Component Model, the shim module doesn't have memory - the main module does.
    /// This function first checks the given instance, then searches other instances.
    #[cfg(all(feature = "std", feature = "wasi"))]
    fn find_instance_with_memory(&self, preferred_instance_id: usize) -> Result<crate::module::MemoryWrapper> {
        // First, try the preferred instance
        if let Some(instance) = self.instances.get(&preferred_instance_id) {
            if let Ok(memory) = instance.memory(0) {
                #[cfg(feature = "tracing")]
                trace!(
                    instance_id = preferred_instance_id,
                    "[CANON_LIFT] Found memory in preferred instance"
                );
                return Ok(memory);
            }
        }

        // Search all instances for one with memory
        // Typically the main module (instance 2 in component model) has memory
        for (&id, instance) in &self.instances {
            if let Ok(memory) = instance.memory(0) {
                #[cfg(feature = "tracing")]
                trace!(
                    instance_id = id,
                    "[CANON_LIFT] Found memory in instance"
                );
                return Ok(memory);
            }
        }

        Err(wrt_error::Error::runtime_error("No instance with memory found for ABI lifting"))
    }

    /// Register a cross-instance import link
    #[cfg(feature = "std")]
    pub fn register_import_link(
        &mut self,
        instance_id: usize,
        import_module: String,
        import_name: String,
        target_instance_id: usize,
        export_name: String,
    ) {
        let key = (instance_id, import_module, import_name);
        self.import_links.insert(key, (target_instance_id, export_name));
    }

    /// Call an exported function in another instance by name
    #[cfg(feature = "std")]
    fn call_exported_function(
        &mut self,
        target_instance_id: usize,
        export_name: &str,
        args: Vec<Value>,
    ) -> Result<Vec<Value>> {
        #[cfg(feature = "tracing")]
        trace!(
            target_instance_id = target_instance_id,
            export_name = %export_name,
            "[CROSS_CALL] call_exported_function"
        );

        // Early depth check to prevent native stack overflow from recursive calls
        if self.call_frames_count >= MAX_CALL_DEPTH {
            #[cfg(feature = "tracing")]
            trace!("[CROSS_CALL] call stack exhausted at depth {} (target_instance={}, export='{}')",
                     self.call_frames_count, target_instance_id, export_name);
            return Err(wrt_error::Error::runtime_trap("call stack exhausted"));
        }

        // Get the target instance
        let target_instance = self.instances.get(&target_instance_id)
            .ok_or_else(|| wrt_error::Error::resource_not_found("Target instance not found"))?
            .clone();

        // Access module via public API
        let module = target_instance.module();

        #[cfg(feature = "tracing")]
        {
            trace!(exports_len = module.exports.len(), "[CROSS_CALL] Target module exports");
            debug!("Module has {} memories", module.memories.len());
            if module.memories.is_empty() {
                warn!("Module has no memories! This will cause memory access errors.");
            }
        }

        // Find the exported function by name
        let mut func_idx = None;
        for (name, export) in module.exports.iter() {
            // BoundedString::as_str() returns Result<&str, BoundedError>
            if let Ok(name_str) = name.as_str() {
                if name_str == export_name {
                    // Export has kind: ExportKind and index: u32 fields
                    use crate::module::ExportKind;
                    if let ExportKind::Function = export.kind {
                        func_idx = Some(export.index as usize);
                        break;
                    }
                }
            }
        }

        let func_idx = func_idx.ok_or_else(|| {
            #[cfg(feature = "tracing")]
            warn!("Cross-instance call: Export '{}' not found", export_name);
            wrt_error::Error::resource_not_found("Export not found")
        })?;

        #[cfg(feature = "tracing")]
        trace!(
            export_name = %export_name,
            func_idx = func_idx,
            target_instance_id = target_instance_id,
            "[CROSS_CALL] Export mapped to function"
        );

        // Execute the function in the target instance
        self.execute(target_instance_id, func_idx, args)
    }

    /// Resolve an exported function to (instance_id, func_idx) without executing it.
    /// Used by the trampoline to redirect import calls without recursion.
    #[cfg(feature = "std")]
    fn resolve_export_func_idx(
        &self,
        target_instance_id: usize,
        export_name: &str,
    ) -> Result<usize> {
        let target_instance = self.instances.get(&target_instance_id)
            .ok_or_else(|| wrt_error::Error::resource_not_found("Target instance not found"))?;
        let module = target_instance.module();

        let mut func_idx = None;
        for (name, export) in module.exports.iter() {
            if let Ok(name_str) = name.as_str() {
                if name_str == export_name {
                    use crate::module::ExportKind;
                    if let ExportKind::Function = export.kind {
                        func_idx = Some(export.index as usize);
                        break;
                    }
                }
            }
        }

        func_idx.ok_or_else(|| {
            wrt_error::Error::resource_not_found("Export not found")
        })
    }

    /// Search for a try_table exception handler in a suspended frame's block_stack.
    ///
    /// This is used by the trampoline's error path to unwind through pending frames
    /// looking for exception handlers. If a handler is found, the frame's state is
    /// modified in place (operand_stack, block_stack, block_depth, pc) so that resuming
    /// the frame will execute the handler code.
    ///
    /// Returns true if a handler was found and applied, false otherwise.
    #[cfg(feature = "std")]
    fn find_and_apply_exception_handler(&mut self, frame: &mut SuspendedFrame) -> bool {
        use wrt_foundation::types::Instruction;

        // Get the active exception (including tag identity for imported tags)
        let (ex_tag_idx, ex_identity, ex_payload) = match &self.active_exception {
            Some((_inst_id, tag_idx, identity, payload)) => (*tag_idx, identity.clone(), payload.clone()),
            None => return false,
        };

        // Look up the module and instructions for this frame
        let instance = match self.instances.get(&frame.instance_id) {
            Some(inst) => inst.clone(),
            None => return false,
        };

        // Check if function is aliased - get the correct module
        let actual_module = if let Some(&original_instance_id) = self.aliased_functions.get(&(frame.instance_id, frame.func_idx)) {
            match self.instances.get(&original_instance_id) {
                Some(inst) => inst.module().clone(),
                None => return false,
            }
        } else {
            instance.module().clone()
        };

        // Get function and instructions
        let func = match actual_module.functions.get(frame.func_idx) {
            Some(f) => f,
            None => return false,
        };
        let instructions = &func.body.instructions;

        // Search for try_table handler in frame's block_stack
        let mut found_handler = false;
        let mut handler_label = 0u32;
        let mut handler_is_ref = false;
        let mut try_block_idx = 0usize;

        for (idx, (block_type, try_pc, _, _)) in frame.block_stack.iter().enumerate().rev() {
            if *block_type == "try_table" {
                if let Some(try_instr) = instructions.get(*try_pc) {
                    if let Instruction::TryTable { handlers, .. } = try_instr {
                        for i in 0..handlers.len() {
                            if let Ok(handler) = handlers.get(i) {
                                let handler_val = handler.clone();
                                let (matches, lbl, is_ref) = match handler_val {
                                    wrt_foundation::types::CatchHandler::Catch { tag_idx: htag, label: hlbl } => {
                                        // Get the handler tag's effective identity
                                        let handler_identity = self.get_effective_tag_identity(frame.instance_id, &actual_module, htag);
                                        // Match by identity if both have identity, by index if both local-only
                                        let tag_matches = match (&ex_identity, &handler_identity) {
                                            (Some(ex_id), Some(h_id)) => ex_id == h_id,
                                            (None, None) => htag == ex_tag_idx,
                                            _ => false,
                                        };
                                        (tag_matches, hlbl, false)
                                    }
                                    wrt_foundation::types::CatchHandler::CatchRef { tag_idx: htag, label: hlbl } => {
                                        // Get the handler tag's effective identity
                                        let handler_identity = self.get_effective_tag_identity(frame.instance_id, &actual_module, htag);
                                        // Match by identity if both have identity, by index if both local-only
                                        let tag_matches = match (&ex_identity, &handler_identity) {
                                            (Some(ex_id), Some(h_id)) => ex_id == h_id,
                                            (None, None) => htag == ex_tag_idx,
                                            _ => false,
                                        };
                                        (tag_matches, hlbl, true)
                                    }
                                    wrt_foundation::types::CatchHandler::CatchAll { label } => {
                                        (true, label, false)
                                    }
                                    wrt_foundation::types::CatchHandler::CatchAllRef { label } => {
                                        (true, label, true)
                                    }
                                };
                                if matches {
                                    handler_label = lbl;
                                    handler_is_ref = is_ref;
                                    found_handler = true;
                                    try_block_idx = idx;
                                    break;
                                }
                            }
                        }
                        if found_handler {
                            break;
                        }
                    }
                }
            }
        }

        if !found_handler {
            return false;
        }

        // Clear active exception - it's being handled
        let payload = ex_payload;
        self.active_exception = None;

        // Save original block count for depth calculation
        let original_block_count = frame.block_stack.len();

        // Pop blocks down to the try_table (but keep the try_table)
        while frame.block_stack.len() > try_block_idx + 1 {
            frame.block_stack.pop();
            frame.block_depth -= 1;
        }

        // Calculate target block index
        // handler_label is relative to try_table's PARENT
        let target_block_idx = if handler_label as usize + 1 <= try_block_idx {
            try_block_idx - 1 - handler_label as usize
        } else {
            // Branch out of function - push payload and set pc past end
            for val in payload.iter() {
                frame.operand_stack.push(val.clone());
            }
            if handler_is_ref {
                let exn_idx = self.exception_storage.len() as u32;
                self.exception_storage.push((ex_tag_idx, ex_identity.clone(), payload));
                frame.operand_stack.push(Value::ExnRef(Some(exn_idx)));
            }
            // Set pc past end of instructions so the resumed function returns immediately
            frame.pc = instructions.len();
            return true;
        };

        // Get entry stack height from target block
        let (_, _, _, entry_stack_height) = frame.block_stack[target_block_idx];

        // Pop blocks down to (but not including) target block
        while frame.block_stack.len() > target_block_idx + 1 {
            frame.block_stack.pop();
            frame.block_depth -= 1;
        }

        // Truncate operand stack to target height
        while frame.operand_stack.len() > entry_stack_height {
            frame.operand_stack.pop();
        }

        // Push payload values
        for val in payload.iter() {
            frame.operand_stack.push(val.clone());
        }
        if handler_is_ref {
            let exn_idx = self.exception_storage.len() as u32;
            self.exception_storage.push((ex_tag_idx, ex_identity, payload));
            frame.operand_stack.push(Value::ExnRef(Some(exn_idx)));
        }

        // Find End of target block to set resume pc
        let ends_to_skip = original_block_count - 1 - target_block_idx;
        let mut depth = ends_to_skip as i32;
        let mut new_pc = frame.pc; // frame.pc points to instruction after the Call
        while new_pc < instructions.len() && depth >= 0 {
            match &instructions[new_pc] {
                Instruction::Block { .. } | Instruction::Loop { .. } | Instruction::If { .. } | Instruction::TryTable { .. } | Instruction::Try { .. } => {
                    depth += 1;
                }
                Instruction::End => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            new_pc += 1;
        }
        frame.pc = new_pc;

        true
    }

    /// Set the host function registry for imported function calls
    #[cfg(feature = "std")]
    pub fn set_host_registry(&mut self, registry: Arc<wrt_host::CallbackRegistry>) {
        self.host_registry = Some(registry);
    }

    /// Add an import link for cross-instance calls
    #[cfg(feature = "std")]
    pub fn add_import_link(
        &mut self,
        instance_id: usize,
        import_module: String,
        import_name: String,
        target_instance_id: usize,
        export_name: String,
    ) {
        self.import_links.insert(
            (instance_id, import_module, import_name),
            (target_instance_id, export_name)
        );
    }

    /// Remap import links from old_id to new_id
    /// This is needed when link_import is called with module_id before instantiation,
    /// but runtime lookup uses instance_id (which is assigned during instantiation)
    #[cfg(feature = "std")]
    pub fn remap_import_links(&mut self, old_id: usize, new_id: usize) {
        if old_id == new_id {
            return;
        }

        // Collect links to remap
        let links_to_remap: Vec<((usize, String, String), (usize, String))> = self
            .import_links
            .iter()
            .filter(|((id, _, _), _)| *id == old_id)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        #[cfg(feature = "tracing")]
        if !links_to_remap.is_empty() {
            tracing::debug!(
                old_id = old_id,
                new_id = new_id,
                count = links_to_remap.len(),
                "Remapping import links"
            );
        }

        // Re-insert with new instance_id
        for ((_, import_module, import_name), (target_id, export_name)) in links_to_remap {
            self.import_links.remove(&(old_id, import_module.clone(), import_name.clone()));
            self.import_links.insert(
                (new_id, import_module, import_name),
                (target_id, export_name),
            );
        }
    }

    /// Register an aliased function origin
    /// This tracks which instance a function actually belongs to when it's aliased
    #[cfg(feature = "std")]
    pub fn register_aliased_function(
        &mut self,
        instance_id: usize,
        func_idx: usize,
        original_instance_id: usize,
    ) {
        self.aliased_functions.insert((instance_id, func_idx), original_instance_id);

        #[cfg(feature = "tracing")]
        debug!(
            "Registered aliased function: instance {} func {} -> original instance {}",
            instance_id, func_idx, original_instance_id
        );
    }

    /// Read LEB128 unsigned 32-bit integer
    fn read_leb128_u32(&self, data: &[u8], offset: usize) -> Result<(u32, usize)> {
        let mut result = 0u32;
        let mut shift = 0;
        let mut bytes_read = 0;

        for i in 0..5 {
            if offset + i >= data.len() {
                return Err(wrt_error::Error::parse_error("Unexpected end of LEB128"));
            }
            let byte = data[offset + i];
            result |= ((byte & 0x7F) as u32) << shift;
            bytes_read += 1;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }

        Ok((result, bytes_read))
    }

    /// Read LEB128 signed 32-bit integer
    fn read_leb128_i32(&self, data: &[u8], offset: usize) -> Result<(i32, usize)> {
        let mut result = 0i32;
        let mut shift = 0;
        let mut bytes_read = 0;
        let mut byte = 0u8;

        for i in 0..5 {
            if offset + i >= data.len() {
                return Err(wrt_error::Error::parse_error("Unexpected end of LEB128"));
            }
            byte = data[offset + i];
            result |= ((byte & 0x7F) as i32) << shift;
            bytes_read += 1;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }

        // Sign extend if necessary
        if shift < 32 && (byte & 0x40) != 0 {
            result |= !0 << shift;
        }

        Ok((result, bytes_read))
    }

    /// Read LEB128 signed 64-bit integer
    fn read_leb128_i64(&self, data: &[u8], offset: usize) -> Result<(i64, usize)> {
        let mut result = 0i64;
        let mut shift = 0;
        let mut bytes_read = 0;
        let mut byte = 0u8;

        for i in 0..10 {
            if offset + i >= data.len() {
                return Err(wrt_error::Error::parse_error("Unexpected end of LEB128"));
            }
            byte = data[offset + i];
            result |= ((byte & 0x7F) as i64) << shift;
            bytes_read += 1;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }

        // Sign extend if necessary
        if shift < 64 && (byte & 0x40) != 0 {
            result |= !0 << shift;
        }

        Ok((result, bytes_read))
    }

    /// Create a new stackless engine (no_std version)
    #[cfg(not(any(feature = "std", feature = "alloc")))]
    pub fn new() -> wrt_error::Result<Self> {
        use wrt_foundation::{
            budget_aware_provider::CrateId,
            safe_managed_alloc,
        };

        let provider = safe_managed_alloc!(4096, CrateId::Runtime)?;
        let instances = BoundedMap::new(provider.clone())
            .map_err(|_| wrt_error::Error::runtime_error("Failed to create instances map"))?;
        let operand_stack = BoundedVec::new(provider)
            .map_err(|_| wrt_error::Error::runtime_error("Failed to create operand stack"))?;

        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            Ok(Self {
                instances:           HashMap::new(),
                next_instance_id:    AtomicU64::new(1),
                current_instance_id: None,
                operand_stack:       Vec::new(),
                call_frames_count:   0,
                stats:               ExecutionStats::default(),
                fuel:                AtomicU64::new(u64::MAX),
                instruction_pointer: AtomicU64::new(0),
            })
        }

        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            Ok(Self {
                instances,
                next_instance_id: AtomicU64::new(1),
                current_instance_id: None,
                operand_stack,
                call_frames_count: 0,
                stats: ExecutionStats::default(),
                fuel: AtomicU64::new(u64::MAX),
                instruction_pointer: AtomicU64::new(0),
            })
        }
    }

    /// Set the current module for execution
    ///
    /// Returns the instance ID that can be used for execution
    pub fn set_current_module(&mut self, instance: Arc<ModuleInstance>) -> Result<usize> {
        let instance_id = self.next_instance_id.fetch_add(1, Ordering::Relaxed) as usize;

        // Check instance limit manually
        if self.instances.len() >= MAX_CONCURRENT_INSTANCES {
            return Err(wrt_error::Error::resource_limit_exceeded(
                "Too many concurrent instances",
            ));
        }

        self.instances.insert(instance_id, instance.clone());

        // Initialize WASI stub memory for this instance
        #[cfg(feature = "tracing")]
        debug!("Attempting to initialize WASI stubs for instance {}", instance_id);
        let module = instance.module();
        match self.initialize_wasi_stubs(instance_id, module) {
            Ok(_) => {
                #[cfg(feature = "tracing")]
                info!("✓ Successfully initialized WASI stubs for instance {}", instance_id);
            },
            Err(e) => {
                #[cfg(feature = "tracing")]
                warn!("Failed to initialize WASI stubs: {:?}", e);
                // Continue anyway - not all modules need WASI
            }
        }

        self.current_instance_id = Some(instance_id);
        Ok(instance_id)
    }

    /// Reset call depth counter to 0
    ///
    /// Call this before each top-level function invocation to ensure
    /// previous errors don't cause false "call stack exhausted" errors.
    pub fn reset_call_depth(&mut self) {
        self.call_frames_count = 0;
    }

    /// Clear all loaded module instances
    ///
    /// This is useful for WAST testing where many modules are loaded in sequence
    /// and we want to avoid hitting instance limits. Note that this invalidates
    /// all existing instance IDs.
    pub fn clear_instances(&mut self) {
        self.instances.clear();
        // Reset instance ID counter to avoid confusion with old IDs
        self.next_instance_id.store(0, Ordering::Relaxed);
    }

    /// Get an instance by ID
    ///
    /// Returns a reference to the ModuleInstance if found.
    #[cfg(any(feature = "std", feature = "alloc"))]
    pub fn get_instance(&self, instance_id: usize) -> Option<Arc<ModuleInstance>> {
        self.instances.get(&instance_id).cloned()
    }

    /// Execute a function in the specified instance
    ///
    /// # Arguments
    /// * `instance_id` - The instance ID returned from set_current_module
    /// * `func_idx` - The function index to execute
    /// * `args` - Function arguments
    ///
    /// # Returns
    /// The function results
    #[cfg(any(feature = "std", feature = "alloc"))]
    pub fn execute(
        &mut self,
        instance_id: usize,
        func_idx: usize,
        args: Vec<Value>,
    ) -> Result<Vec<Value>> {
        // Full call trampoline: handles TailCall, regular Call, and exception unwinding.
        // Instead of recursive Rust stack frames (which overflow in debug mode at ~50-100
        // levels due to the ~100-160KB frame size of execute_function_body), we maintain
        // an explicit pending_frames stack on the heap. This gives us O(1) Rust stack
        // usage per wasm-to-wasm call.
        let mut current_instance_id = instance_id;
        let mut current_func_idx = func_idx;
        let mut current_args = args;
        let mut pending_frames: Vec<SuspendedFrame> = Vec::new();
        let mut resume_state: Option<SuspendedFrame> = None;

        // Check call depth for the initial function
        if self.call_frames_count >= MAX_CALL_DEPTH {
            return Err(wrt_error::Error::runtime_trap("call stack exhausted"));
        }
        self.call_frames_count += 1;

        loop {
            let outcome = self.execute_function_body(
                current_instance_id,
                current_func_idx,
                std::mem::take(&mut current_args),
                resume_state.take(),
            );

            match outcome {
                Ok(ExecutionOutcome::Complete(results)) => {
                    if let Some(mut frame) = pending_frames.pop() {
                        // Callee completed - push results onto caller's operand stack
                        self.call_frames_count = self.call_frames_count.saturating_sub(1);
                        for result in results {
                            frame.operand_stack.push(result);
                        }
                        // Resume the caller
                        current_instance_id = frame.instance_id;
                        current_func_idx = frame.func_idx;
                        current_args = Vec::new(); // unused when resuming
                        resume_state = Some(frame);
                    } else {
                        // Top-level return - trampoline is done
                        self.call_frames_count = self.call_frames_count.saturating_sub(1);
                        return Ok(results);
                    }
                }
                Ok(ExecutionOutcome::TailCall { func_idx: next_func, args: next_args }) => {
                    // Tail call - reuse current frame slot, no stack growth
                    current_func_idx = next_func;
                    current_args = next_args;
                    resume_state = None;
                }
                Ok(ExecutionOutcome::Call {
                    instance_id: target_id,
                    func_idx: target_func,
                    args: call_args,
                    return_state,
                }) => {
                    // Regular call - save caller state and set up callee
                    if let Some(state) = return_state {
                        pending_frames.push(state);
                    }
                    // Check call depth for the new callee
                    if self.call_frames_count >= MAX_CALL_DEPTH {
                        let depth = pending_frames.len() + 1;
                        self.call_frames_count = self.call_frames_count.saturating_sub(depth);
                        return Err(wrt_error::Error::runtime_trap("call stack exhausted"));
                    }
                    self.call_frames_count += 1;
                    current_instance_id = target_id;
                    current_func_idx = target_func;
                    current_args = call_args;
                    resume_state = None;
                }
                Err(e) => {
                    // Handle exception unwinding through pending frames
                    #[cfg(feature = "std")]
                    if self.active_exception.is_some() {
                        // Current function (which errored) is done
                        self.call_frames_count = self.call_frames_count.saturating_sub(1);
                        let mut found_handler = false;
                        while let Some(mut frame) = pending_frames.pop() {
                            if self.find_and_apply_exception_handler(&mut frame) {
                                // Handler found - resume this frame
                                current_instance_id = frame.instance_id;
                                current_func_idx = frame.func_idx;
                                current_args = Vec::new();
                                resume_state = Some(frame);
                                found_handler = true;
                                break;
                            }
                            // No handler in this frame - unwind it
                            self.call_frames_count = self.call_frames_count.saturating_sub(1);
                        }
                        if found_handler {
                            continue;
                        }
                        // No handler found in any frame
                        return Err(e);
                    }
                    // Non-exception error - clean up all frames
                    let depth = pending_frames.len() + 1;
                    self.call_frames_count = self.call_frames_count.saturating_sub(depth);
                    return Err(e);
                }
            }
        }
    }

    /// Execute a leaf function that is guaranteed not to make further calls.
    /// Used for cabi_realloc and similar canonical ABI functions that only do
    /// memory operations and return immediately. This avoids creating a nested
    /// trampoline when called from within execute_function_body.
    ///
    /// # Panics
    /// Panics if the function attempts a Call or TailCall (violating leaf contract).
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn execute_leaf_function(
        &mut self,
        instance_id: usize,
        func_idx: usize,
        args: Vec<Value>,
    ) -> Result<Vec<Value>> {
        // Leaf functions still count toward call depth for safety
        if self.call_frames_count >= MAX_CALL_DEPTH {
            return Err(wrt_error::Error::runtime_trap("call stack exhausted"));
        }
        self.call_frames_count += 1;

        // Execute the function body directly - no trampoline loop needed
        let outcome = self.execute_function_body(instance_id, func_idx, args, None);

        self.call_frames_count = self.call_frames_count.saturating_sub(1);

        match outcome {
            Ok(ExecutionOutcome::Complete(results)) => Ok(results),
            Ok(ExecutionOutcome::TailCall { .. }) => {
                // Leaf functions must not tail call
                Err(wrt_error::Error::runtime_error(
                    "leaf function attempted tail call (cabi_realloc contract violation)",
                ))
            }
            Ok(ExecutionOutcome::Call { .. }) => {
                // Leaf functions must not call other functions
                Err(wrt_error::Error::runtime_error(
                    "leaf function attempted call (cabi_realloc contract violation)",
                ))
            }
            Err(e) => Err(e),
        }
    }

    /// Internal function body execution - can return TailCall for trampolining
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn execute_function_body(
        &mut self,
        instance_id: usize,
        func_idx: usize,
        args: Vec<Value>,
        resume: Option<SuspendedFrame>,
    ) -> Result<ExecutionOutcome> {
        #[cfg(feature = "tracing")]
        let _span = ExecutionTrace::function(func_idx, instance_id).entered();

        // Clone the instance to avoid holding a borrow on self.instances
        // This allows us to call &mut self methods (like execute, call_wasi_function)
        // during execution without borrow checker conflicts.
        #[cfg(any(feature = "std", feature = "alloc"))]
        let instance = {
            let found = self.instances.get(&instance_id);
            if found.is_none() {
                #[cfg(feature = "tracing")]
                trace!(instance_id = instance_id, "[INNER_EXEC] Instance not found");
                return Err(wrt_error::Error::runtime_execution_error("Instance not found"));
            }
            found.unwrap().clone()
        };

        #[cfg(not(any(feature = "std", feature = "alloc")))]
        let instance = self
            .instances
            .get(&instance_id)?
            .ok_or_else(|| wrt_error::Error::runtime_execution_error("Instance not found"))?
            .clone();

        // Check if this function is aliased and get the correct module
        // Clone the Arc<Module> so we own it and don't hold a borrow
        #[cfg(feature = "std")]
        let module: Arc<crate::module::Module> = {
            if let Some(&original_instance_id) = self.aliased_functions.get(&(instance_id, func_idx)) {
                #[cfg(feature = "tracing")]
                debug!(
                    "Function {} in instance {} is aliased from instance {}",
                    func_idx, instance_id, original_instance_id
                );

                // Get the original instance that actually contains the function
                let original_instance = self
                    .instances
                    .get(&original_instance_id)
                    .ok_or_else(|| wrt_error::Error::runtime_execution_error("Original instance not found"))?;
                original_instance.module().clone()
            } else {
                // Not aliased, use the current instance's module
                instance.module().clone()
            }
        };

        #[cfg(not(feature = "std"))]
        let module: Arc<crate::module::Module> = instance.module().clone();

        #[cfg(feature = "std")]
        {
            let elem_count = module.elements.len();
            let first_elem_items = if elem_count > 0 {
                // In std mode, elements is Vec, so get() returns Option<&T>
                if let Some(elem) = module.elements.get(0) {
                    elem.items.len()
                } else {
                    0
                }
            } else {
                0
            };
            #[cfg(feature = "tracing")]
            trace!(
                instance_id = instance_id,
                func_idx = func_idx,
                elem_count = elem_count,
                first_elem_items = first_elem_items,
                "[EXECUTE] Starting function execution"
            );
        }

        // Check if this function index is an import and dispatch to linked instance
        let num_imports = self.count_total_imports(&module);
        if func_idx < num_imports {
            #[cfg(feature = "std")]
            {
                // This is an imported function - need to call via import link
                if let Ok((module_name, field_name)) = self.find_import_by_index(&module, func_idx) {
                    let import_key = (instance_id, module_name.clone(), field_name.clone());
                    if let Some((target_instance_id, export_name)) = self.import_links.get(&import_key)
                        .map(|(ti, en)| (*ti, en.clone()))
                    {
                        // Detect self-referencing loop
                        let target_func = self.resolve_export_func_idx(target_instance_id, &export_name)?;
                        if target_instance_id == instance_id && target_func == func_idx {
                            return Err(wrt_error::Error::runtime_trap("circular import link detected"));
                        }
                        // NON-RECURSIVE: Resolve target and redirect via trampoline
                        return Ok(ExecutionOutcome::Call {
                            instance_id: target_instance_id,
                            func_idx: target_func,
                            args,
                            return_state: None, // No state to save - this is the very start
                        });
                    }
                    // Import not linked - return correct number of default results
                    // based on the imported function's type signature
                    // NOTE: Do NOT decrement here - execute() will decrement on Complete
                    if let Some(func) = module.functions.get(func_idx) {
                        if let Some(func_type) = module.types.get(func.type_idx as usize) {
                            let mut results = Vec::new();
                            for result_type in &func_type.results {
                                let default_value = match result_type {
                                    wrt_foundation::ValueType::I32 => Value::I32(0),
                                    wrt_foundation::ValueType::I64 => Value::I64(0),
                                    wrt_foundation::ValueType::F32 => Value::F32(FloatBits32(0)),
                                    wrt_foundation::ValueType::F64 => Value::F64(FloatBits64(0)),
                                    _ => Value::I32(0),
                                };
                                results.push(default_value);
                            }
                            return Ok(ExecutionOutcome::Complete(results));
                        }
                    }
                    // Fallback for corrupted module data
                    return Ok(ExecutionOutcome::Complete(Vec::new()));
                }
            }
            #[cfg(not(feature = "std"))]
            {
                // In no_std mode, return error for unresolved imports
                self.call_frames_count -= 1;
                return Err(wrt_error::Error::runtime_error("Import resolution not supported in no_std"));
            }
        }

        // Validate function index
        // Note: module.functions includes both import stubs and local functions
        if func_idx >= module.functions.len() {
            return Err(wrt_error::Error::runtime_function_not_found(
                "Function index out of bounds",
            ));
        }

        // Get function type to determine return values
        // Note: module.functions includes both import stubs and local functions
        let func = module
            .functions
            .get(func_idx)
            .ok_or_else(|| wrt_error::Error::runtime_function_not_found("Failed to get function"))?;

        #[cfg(feature = "std")]
        #[cfg(feature = "tracing")]

        debug!("StacklessEngine: func.type_idx={}, module.types.len()={}", func.type_idx, module.types.len());

        // In std mode, types is Vec so use simple indexing
        #[cfg(feature = "std")]
        let func_type = module
            .types
            .get(func.type_idx as usize)
            .ok_or_else(|| wrt_error::Error::runtime_error("Function type index out of bounds"))?;

        // In no_std mode, types is BoundedVec so use .get() method
        #[cfg(not(feature = "std"))]
        let func_type = module
            .types
            .get(func.type_idx as usize)
            .map_err(|e| {
                #[cfg(feature = "tracing")]

                debug!("StacklessEngine: Failed to get type at index {}: {:?}", func.type_idx, e);
                wrt_error::Error::runtime_error("Failed to get function type")
            })?;

        // Execute the function's bytecode instructions
        #[cfg(feature = "std")]
        {
            use wrt_foundation::types::Instruction;

            // Get the parsed instructions
            #[cfg(feature = "tracing")]

            debug!("Accessing func.body for func_idx={}", func_idx);
            #[cfg(feature = "tracing")]

            debug!("func.type_idx={}, func.locals.len()={}", func.type_idx, func.locals.len());

            // Get the function type to see how many parameters it expects
            if let Some(func_type) = module.types.get(func.type_idx as usize) {
                #[cfg(feature = "tracing")]

                debug!("Function type: params.len()={}, results.len()={}",                          func_type.params.len(), func_type.results.len());
            }
            #[cfg(feature = "tracing")]

            debug!("Called with args.len()={}", args.len());

            let instructions = &func.body.instructions;
            #[cfg(feature = "tracing")]
            trace!(
                func_idx = func_idx,
                instance_id = instance_id,
                instructions_len = instructions.len(),
                "[EXEC] Starting execution"
            );
            // Take debugger out of self to use during execution (avoids borrow issues)
            // This is done for both fresh calls and resumes - the debugger is restored
            // before returning Call outcomes and re-taken on resume.
            #[cfg(all(feature = "std", feature = "debugger"))]
            let mut debugger_opt = self.debugger.take();

            // Save the current function index for SuspendedFrame construction.
            // Inside the instruction match, Instruction::Call(func_idx) shadows this,
            // so we need a separate binding for the caller's func_idx.
            let caller_func_idx = func_idx;

            // Initialize execution state - either from resume or fresh call
            let mut operand_stack: Vec<Value>;
            let mut locals: Vec<Value>;
            let mut instruction_count: usize;
            let mut block_depth: i32;
            let mut pc: usize;
            let mut block_stack: Vec<(&'static str, usize, u32, usize)>;

            if let Some(frame) = resume {
                // Resume from suspended state - use saved values directly (moved, not cloned)
                operand_stack = frame.operand_stack;
                locals = frame.locals;
                block_stack = frame.block_stack;
                block_depth = frame.block_depth;
                instruction_count = frame.instruction_count;
                pc = frame.pc;
            } else {
                // Fresh call - initialize from args and function signature
                operand_stack = Vec::new();
                locals = Vec::new();
                instruction_count = 0;
                block_depth = 0;
                pc = 0;
                block_stack = Vec::new();

                // Initialize parameters as locals
                // Need to match the function type signature, not just provided args
                #[cfg(feature = "tracing")]
                trace!("Initializing locals: args.len()={}, func.locals.len()={}", args.len(), func.locals.len());

                // Get expected parameter count from function type
                // Per CLAUDE.md: FAIL LOUD AND EARLY - don't silently substitute defaults
                let func_type = module.types.get(func.type_idx as usize)
                    .ok_or_else(|| wrt_error::Error::runtime_error(
                        "Function type not found - module corrupted"
                    ))?;
                let expected_param_count = func_type.params.len();

                #[cfg(feature = "tracing")]
                trace!(
                    expected_param_count = expected_param_count,
                    args_len = args.len(),
                    "[EXEC] Function parameter info"
                );

                // Add provided arguments
                for (i, arg) in args.iter().enumerate() {
                    if i < expected_param_count {
                        locals.push(arg.clone());
                    }
                }

                // Pad with default values for missing parameters
                if args.len() < expected_param_count {
                    for i in args.len()..expected_param_count {
                        let param_type = func_type.params.get(i)
                            .ok_or_else(|| wrt_error::Error::runtime_error(
                                "Parameter index out of bounds - type corrupted"
                            ))?;
                        let default_value = match param_type {
                            wrt_foundation::ValueType::I32 => Value::I32(0),
                            wrt_foundation::ValueType::I64 => Value::I64(0),
                            wrt_foundation::ValueType::F32 => Value::F32(FloatBits32(0)),
                            wrt_foundation::ValueType::F64 => Value::F64(FloatBits64(0)),
                            _ => Value::I32(0),
                        };
                        locals.push(default_value);
                    }
                }

                #[cfg(feature = "tracing")]
                trace!("After parameters: locals.len()={}", locals.len());

                // Initialize remaining locals to zero
                for i in 0..func.locals.len() {
                    if let Ok(local_decl) = func.locals.get(i) {
                        #[cfg(feature = "tracing")]
                        trace!("LocalEntry[{}]: type={:?}, count={}", i, local_decl.value_type, local_decl.count);
                        let zero_value = match local_decl.value_type {
                            wrt_foundation::ValueType::I32 => Value::I32(0),
                            wrt_foundation::ValueType::I64 => Value::I64(0),
                            wrt_foundation::ValueType::F32 => Value::F32(FloatBits32(0)),
                            wrt_foundation::ValueType::F64 => Value::F64(FloatBits64(0)),
                            _ => Value::I32(0),
                        };
                        for _ in 0..local_decl.count {
                            locals.push(zero_value.clone());
                        }
                        #[cfg(feature = "tracing")]
                        trace!("After LocalEntry[{}]: locals.len()={}", i, locals.len());
                    }
                }
                #[cfg(feature = "tracing")]
                trace!("Initialized {} locals total", locals.len());
            } // end of fresh-call initialization

            while pc < instructions.len() {
                #[cfg(feature = "std")]
                let instruction = instructions.get(pc)
                    .ok_or_else(|| wrt_error::Error::runtime_error("Instruction index out of bounds"))?;
                #[cfg(not(feature = "std"))]
                let instruction = instructions.get(pc)
                    .map_err(|_| wrt_error::Error::runtime_error("Instruction index out of bounds"))?;

                instruction_count += 1;
                #[cfg(feature = "tracing")]
                trace!("pc={}, instruction={:?}", pc, instruction);

                // Debugger callback - notify debugger of instruction execution
                #[cfg(all(feature = "std", feature = "debugger"))]
                if let Some(ref mut debugger) = debugger_opt {
                    let state = ExecutionState {
                        pc: pc as u32,
                        func_idx: func_idx as u32,
                        operand_stack: &operand_stack,
                        locals: &locals,
                    };
                    let action = debugger.on_instruction(pc as u32, &state);
                    if action == DebugAction::Break {
                        return Err(wrt_error::Error::runtime_execution_error("Debugger break requested"));
                    }
                }

                match *instruction {
                    Instruction::Unreachable => {
                        // Unreachable instruction - this is a WebAssembly trap
                        // The trap should propagate as an error, not be silently ignored.
                        // This can occur in panic paths when the panic hook is NULL,
                        // or after proc_exit is called.
                        #[cfg(all(feature = "std", feature = "tracing"))]
                        {
                            // Print some context about what led to unreachable
                            let prev_str = if pc > 0 { format!("{:?}", instructions.get(pc - 1)) } else { "N/A".to_string() };
                            error!(
                                func_idx = func_idx,
                                pc = pc,
                                prev_instr = %prev_str,
                                "[TRAP] Unreachable instruction executed"
                            );
                        }
                        return Err(wrt_error::Error::runtime_execution_error(
                            "WebAssembly trap: unreachable instruction executed",
                        ));
                    }
                    Instruction::Nop => {
                        // No operation - do nothing
                        #[cfg(feature = "tracing")]

                        trace!("Nop");
                    }
                    Instruction::Drop => {
                        // Pop and discard top value from stack
                        if let Some(value) = operand_stack.pop() {
                            #[cfg(feature = "tracing")]

                            trace!("Drop: discarded {:?}", value);
                        } else {
                            #[cfg(feature = "tracing")]

                            trace!("Drop: stack underflow");
                            return Err(wrt_error::Error::runtime_trap("Drop: stack underflow"));
                        }
                    }
                    Instruction::Select => {
                        // Pop condition, then two values, push selected value
                        // Stack: [val1, val2, condition] -> [selected]
                        // WebAssembly spec: if condition != 0, select val1 (deeper), else select val2 (higher)
                        #[cfg(feature = "tracing")]
                        trace!(stack_len = operand_stack.len(), "[Select] Processing select instruction");

                        // WebAssembly select expects: val1, val2, i32
                        // The condition should be the top of stack
                        let cond_val = operand_stack.pop();
                        let val2 = operand_stack.pop();
                        let val1 = operand_stack.pop();

                        #[cfg(feature = "tracing")]
                        trace!(cond = ?cond_val, val2 = ?val2, val1 = ?val1, "[Select] Popped values");

                        // Extract condition as i32
                        let condition = match cond_val {
                            Some(Value::I32(c)) => c,
                            Some(other) => {
                                #[cfg(feature = "tracing")]
                                error!(condition = ?other, "[Select] ERROR: condition is not i32");
                                return Err(wrt_error::Error::runtime_trap("Select: condition must be i32"));
                            }
                            None => return Err(wrt_error::Error::runtime_trap("Select: stack underflow (no condition)")),
                        };

                        if let (Some(v2), Some(v1)) = (val2, val1) {
                            // WebAssembly spec: if cond != 0, select val1 (pushed first)
                            // val1 is deeper on stack, val2 is higher
                            let selected = if condition != 0 { v1 } else { v2 };
                            #[cfg(feature = "tracing")]
                            trace!("Select: condition={}, selected={:?}", condition, selected);
                            #[cfg(feature = "tracing")]
                            {
                                if let Value::I32(v) = &selected {
                                    if (*v as u32) > 0x20000 {
                                        warn!(
                                            condition = condition,
                                            value = v,
                                            value_hex = format_args!("0x{:x}", *v as u32),
                                            "[Select] SUSPICIOUS: large value selected"
                                        );
                                    }
                                }
                            }
                            operand_stack.push(selected);
                        } else {
                            #[cfg(feature = "tracing")]

                            trace!("Select: insufficient operands on stack");
                            return Err(wrt_error::Error::runtime_trap("Select: stack underflow (missing values)"));
                        }
                    }
                    Instruction::SelectWithType(ref _types) => {
                        // Typed select - semantically identical to untyped select at runtime
                        // The type annotation is for validation purposes only
                        // Stack: [val1, val2, condition] -> [selected]
                        #[cfg(feature = "tracing")]
                        trace!(stack_len = operand_stack.len(), "[SelectWithType] Processing typed select instruction");

                        let cond_val = operand_stack.pop();
                        let val2 = operand_stack.pop();
                        let val1 = operand_stack.pop();

                        #[cfg(feature = "tracing")]
                        trace!(cond = ?cond_val, val2 = ?val2, val1 = ?val1, "[SelectWithType] Popped values");

                        // Extract condition as i32
                        let condition = match cond_val {
                            Some(Value::I32(c)) => c,
                            Some(other) => {
                                #[cfg(feature = "tracing")]
                                error!(condition = ?other, "[SelectWithType] ERROR: condition is not i32");
                                return Err(wrt_error::Error::runtime_trap("SelectWithType: condition must be i32"));
                            }
                            None => return Err(wrt_error::Error::runtime_trap("SelectWithType: stack underflow (no condition)")),
                        };

                        if let (Some(v2), Some(v1)) = (val2, val1) {
                            // WebAssembly spec: if cond != 0, select val1 (pushed first)
                            let selected = if condition != 0 { v1 } else { v2 };
                            #[cfg(feature = "tracing")]
                            trace!("SelectWithType: condition={}, selected={:?}", condition, selected);
                            operand_stack.push(selected);
                        } else {
                            #[cfg(feature = "tracing")]
                            trace!("SelectWithType: insufficient operands on stack");
                            return Err(wrt_error::Error::runtime_trap("SelectWithType: stack underflow (missing values)"));
                        }
                    }
                    Instruction::Call(func_idx) => {
                        // Count total number of imports across all modules
                        let num_imports = self.count_total_imports(&module);

                        #[cfg(feature = "tracing")]
                        trace!(
                            func_idx = func_idx,
                            num_imports = num_imports,
                            is_import = (func_idx as usize) < num_imports,
                            "[CALL] Checking function call"
                        );

                        if (func_idx as usize) < num_imports {
                            // This is a host function call
                            #[cfg(feature = "tracing")]
                            trace!("[CALL_IMPORT] instance={}, func_idx={}, num_imports={}", instance_id, func_idx, num_imports);

                            // CHECK FOR LOWERED FUNCTION: If this import was created by canon.lower,
                            // dispatch to the canonical executor instead of using import_links.
                            // This prevents infinite recursion when adapter modules import canon-lowered
                            // functions that are backed by InlineExports with no real module.
                            #[cfg(all(feature = "std", feature = "wasi"))]
                            {
                            let is_lowered = self.is_lowered_function(instance_id, func_idx as usize);
                            if is_lowered {
                                #[cfg(feature = "tracing")]
                                trace!(
                                    instance_id = instance_id,
                                    func_idx = func_idx,
                                    "[CALL] Import is a canon.lower synthesized function - dispatching to WASI"
                                );

                                // Collect args from operand stack based on function signature
                                let args = Self::collect_function_args(&module, func_idx as usize, &mut operand_stack);

                                // Execute the lowered function via WASI dispatcher
                                let results = self.execute_lowered_function(instance_id, func_idx as usize, args)?;

                                // Push results back onto stack
                                for result in results {
                                    operand_stack.push(result);
                                }

                                // Skip the normal import handling
                                pc += 1;
                                continue;
                            }
                            } // end #[cfg(all(feature = "std", feature = "wasi"))]

                            // Find the import by index
                            let import_result = self.find_import_by_index(&module, func_idx as usize);
                            #[cfg(feature = "tracing")]
                            trace!(result = ?import_result, "[HOST_CALL] find_import_by_index result");

                            if let Ok((module_name, field_name)) = import_result {
                                #[cfg(feature = "tracing")]
                                trace!(
                                    module_name = %module_name,
                                    field_name = %field_name,
                                    "[HOST_CALL] Resolved to host function"
                                );

                                // Check if this import is linked to another instance
                                // NOTE: Component adapter modules handle P2→P1 translation.
                                // ALL imports (including WASI) should use cross-instance linking when linked.
                                #[cfg(feature = "std")]
                                {
                                    // Clone the values to avoid holding a borrow during call_exported_function
                                    let import_key = (instance_id, module_name.clone(), field_name.clone());
                                    let linked = self.import_links.get(&import_key)
                                        .map(|(ti, en)| (*ti, en.clone()));

                                    #[cfg(feature = "tracing")]
                                    trace!(
                                        instance_id = instance_id,
                                        is_linked = linked.is_some(),
                                        "[HOST_CALL] Checking cross-instance link"
                                    );

                                    if let Some((target_instance, export_name)) = linked {
                                        // Collect args from operand stack based on function signature
                                        let call_args = Self::collect_function_args(&module, func_idx as usize, &mut operand_stack);

                                        // NON-RECURSIVE: resolve target function and return to trampoline
                                        let target_func = self.resolve_export_func_idx(target_instance, &export_name)?;

                                        // Save current execution state for resumption after callee returns
                                        let saved_state = SuspendedFrame {
                                            instance_id,
                                            func_idx: caller_func_idx,
                                            pc: pc + 1, // resume at next instruction
                                            locals,
                                            operand_stack,
                                            block_stack,
                                            block_depth,
                                            instruction_count,
                                        };

                                        return Ok(ExecutionOutcome::Call {
                                            instance_id: target_instance,
                                            func_idx: target_func,
                                            args: call_args,
                                            return_state: Some(saved_state),
                                        });
                                    }
                                    // Not linked - fall through to WASI dispatch
                                }

                                // Dispatch to WASI implementation
                                #[cfg(feature = "tracing")]
                                trace!(
                                    module_name = %module_name,
                                    field_name = %field_name,
                                    "[HOST_CALL] Calling call_wasi_function"
                                );
                                let result = self.call_wasi_function(
                                    &module_name,
                                    &field_name,
                                    &mut operand_stack,
                                    &module,
                                    instance_id,
                                )?;
                                #[cfg(feature = "tracing")]
                                trace!(result = ?result, "[HOST_CALL] call_wasi_function returned");

                                // Push result onto stack if function returns a value
                                if let Some(value) = result {
                                    #[cfg(feature = "tracing")]
                                    {
                                        if let Value::I32(v) = &value {
                                            if (*v as u32) > 0x20000 {
                                                warn!(
                                                    module_name = %module_name,
                                                    field_name = %field_name,
                                                    value = v,
                                                    value_hex = format_args!("0x{:x}", *v as u32),
                                                    "[WASI_RETURN] SUSPICIOUS: large return value"
                                                );
                                            }
                                        }
                                    }
                                    operand_stack.push(value);
                                }
                            } else {
                                #[cfg(feature = "tracing")]

                                trace!("Warning: Could not resolve import {}", func_idx);
                                // Push dummy return value to keep stack balanced
                                operand_stack.push(Value::I32(0));
                            }
                        } else {
                            // Regular function call - get function signature to know how many args to pop
                            // NOTE: module.functions contains ALL functions (imports + defined)
                            // So we use func_idx directly, NOT (func_idx - num_imports)
                            let local_func_idx = func_idx as usize;
                            #[cfg(feature = "tracing")]
                            trace!(
                                func_idx = func_idx,
                                num_imports = num_imports,
                                local_func_idx = local_func_idx,
                                functions_len = module.functions.len(),
                                "[CALL] Regular function call"
                            );
                            if local_func_idx >= module.functions.len() {
                                #[cfg(feature = "tracing")]

                                trace!("Function index {} out of bounds", func_idx);
                                return Err(wrt_error::Error::runtime_error("Function index out of bounds"));
                            }

                            let func = &module.functions[local_func_idx];
                            #[cfg(feature = "tracing")]
                            trace!(
                                type_idx = func.type_idx,
                                types_len = module.types.len(),
                                "[CALL] Function type info"
                            );
                            let func_type = module.types.get(func.type_idx as usize)
                                .ok_or_else(|| wrt_error::Error::runtime_error("Invalid function type"))?;

                            // Pop the required number of arguments from the stack
                            let param_count = func_type.params.len();
                            #[cfg(feature = "tracing")]
                            trace!(
                                param_count = param_count,
                                stack_len = operand_stack.len(),
                                "[CALL] Parameter and stack info"
                            );
                            #[cfg(all(feature = "std", feature = "tracing"))]
                            if func_idx == 94 || func_idx == 223 || func_idx == 232 || func_idx == 233 {
                                trace!(
                                    func_idx = func_idx,
                                    stack_top = ?operand_stack.iter().rev().take(4).collect::<Vec<_>>(),
                                    "[CALL-ALLOC] Allocation function call"
                                );
                            }
                            // Trace func 235 (free) to see what pointer is being freed
                            #[cfg(all(feature = "std", feature = "tracing"))]
                            if func_idx == 235 || func_idx == 236 {
                                trace!(
                                    func_idx = func_idx,
                                    args = ?operand_stack.iter().rev().take(1).collect::<Vec<_>>(),
                                    "[FREE-TRACE] Free function call"
                                );
                            }
                            // Trace func 244 (format string loop) to see arguments
                            #[cfg(all(feature = "std", feature = "tracing"))]
                            if func_idx == 244 {
                                trace!(
                                    args = ?operand_stack.iter().rev().take(8).collect::<Vec<_>>(),
                                    "[CALL-244] Format string loop args"
                                );
                            }

                            #[cfg(feature = "tracing")]
                            trace!("Call({}): needs {} params, stack has {} values", func_idx, param_count, operand_stack.len());

                            let mut call_args = Vec::new();
                            for _ in 0..param_count {
                                if let Some(arg) = operand_stack.pop() {
                                    call_args.push(arg);
                                } else {
                                    #[cfg(feature = "tracing")]

                                    trace!("Not enough arguments on stack for function call");
                                    return Err(wrt_error::Error::runtime_error("Stack underflow on function call"));
                                }
                            }
                            // Arguments were popped in reverse order, so reverse them
                            call_args.reverse();

                            #[cfg(feature = "tracing")]


                            trace!("Stack before call: {} values, after popping args: {} values",                                 operand_stack.len() + call_args.len(), operand_stack.len());

                            // Trampoline: save caller state and return to the trampoline loop.
                            // The trampoline will execute the callee, and when it completes,
                            // push results onto our operand_stack and resume us at pc+1.
                            // Exception handling is managed by the trampoline via
                            // find_and_apply_exception_handler on pending frames.
                            #[cfg(all(feature = "std", feature = "debugger"))]
                            {
                                self.debugger = debugger_opt;
                            }
                            return Ok(ExecutionOutcome::Call {
                                instance_id,
                                func_idx: func_idx as usize,
                                args: call_args,
                                return_state: Some(SuspendedFrame {
                                    instance_id,
                                    func_idx: caller_func_idx,
                                    pc: pc + 1,
                                    locals,
                                    operand_stack,
                                    block_stack,
                                    block_depth,
                                    instruction_count,
                                }),
                            });
                        }
                    }
                    Instruction::CallIndirect(type_idx, table_idx) => {
                        // CallIndirect: call a function through an indirect table reference
                        // Pop the function index from the stack
                        let table_func_idx = if let Some(Value::I32(idx)) = operand_stack.pop() {
                            idx as u32
                        } else {
                            return Err(wrt_error::Error::runtime_trap("CallIndirect: expected i32 function index on stack"));
                        };

                        #[cfg(feature = "tracing")]
                        trace!(
                            type_idx = type_idx,
                            table_idx = table_idx,
                            table_func_idx = table_func_idx,
                            "[CALL_INDIRECT] Indirect call"
                        );

                        // Look up the function in the table
                        // For now, we need to get the table from the instance and look up the function
                        let func_idx = if let Some(inst) = self.instances.get(&instance_id) {
                            // Get the table
                            if let Ok(table) = inst.table(table_idx) {
                                // Get the function reference from the table
                                if let Ok(Some(func_ref)) = table.0.get(table_func_idx) {
                                    // Extract the function index from the Value
                                    // Tables store FuncRef values, not raw integers
                                    match func_ref {
                                        Value::FuncRef(Some(fref)) => fref.index as usize,
                                        Value::FuncRef(None) => return Err(wrt_error::Error::runtime_trap("uninitialized element")),
                                        Value::I32(idx) => idx as usize, // Legacy fallback
                                        Value::I64(idx) => idx as usize, // Legacy fallback
                                        _ => return Err(wrt_error::Error::runtime_trap("uninitialized element")),
                                    }
                                } else if let Ok(None) = table.0.get(table_func_idx) {
                                    return Err(wrt_error::Error::runtime_trap("uninitialized element"));
                                } else {
                                    return Err(wrt_error::Error::runtime_trap("undefined element"));
                                }
                            } else {
                                // Fall back: use the element segment if tables aren't properly initialized
                                // Look through element segments to find the function
                                #[cfg(feature = "tracing")]
                                trace!(table_idx = table_idx, "[CALL_INDIRECT] Table not found, checking element segments");

                                let mut resolved_func_idx: Option<usize> = None;

                                // Search through element segments
                                // Element segments have format: (elem (i32.const offset) func f1 f2 f3 ...)
                                // We need to find which element contains table_func_idx
                                #[cfg(feature = "tracing")]
                                trace!(elements_len = module.elements.len(), "[CALL_INDIRECT] Searching element segments");
                                for elem_idx in 0..module.elements.len() {
                                    // In std mode, elements is Vec, so get() returns Option<&T>
                                    // In no_std mode, elements is BoundedVec, so get() returns Result<T>
                                    #[cfg(feature = "std")]
                                    let elem_opt = module.elements.get(elem_idx);
                                    #[cfg(not(feature = "std"))]
                                    let elem_opt = module.elements.get(elem_idx).ok().as_ref();

                                    if let Some(elem) = elem_opt {
                                        // The offset is where this element starts in the table
                                        // First check mode for offset, then fall back to offset_expr
                                        let elem_offset = match &elem.mode {
                                            wrt_foundation::types::ElementMode::Active { offset, .. } => *offset,
                                            _ => {
                                                // Try offset_expr
                                                if let Some(ref offset_expr) = elem.offset_expr {
                                                    #[cfg(feature = "std")]
                                                    {
                                                        if let Some(Instruction::I32Const(off)) = offset_expr.instructions.first() {
                                                            *off as u32
                                                        } else {
                                                            0
                                                        }
                                                    }
                                                    #[cfg(not(feature = "std"))]
                                                    0
                                                } else {
                                                    0
                                                }
                                            }
                                        };

                                        let items_len = elem.items.len();
                                        #[cfg(feature = "tracing")]
                                        trace!(
                                            elem_idx = elem_idx,
                                            elem_offset = elem_offset,
                                            items_len = items_len,
                                            table_func_idx = table_func_idx,
                                            "[CALL_INDIRECT] Element segment info"
                                        );

                                        // Check if table_func_idx falls within this element's range
                                        if table_func_idx >= elem_offset && (table_func_idx - elem_offset) < items_len as u32 {
                                            let elem_local_idx = (table_func_idx - elem_offset) as usize;
                                            // items is BoundedVec<u32>, get() returns Result<u32>
                                            if let Ok(func_ref) = elem.items.get(elem_local_idx) {
                                                resolved_func_idx = Some(func_ref as usize);
                                                #[cfg(feature = "tracing")]
                                                trace!(
                                                    elem_idx = elem_idx,
                                                    table_func_idx = table_func_idx,
                                                    elem_local_idx = elem_local_idx,
                                                    func_ref = func_ref,
                                                    "[CALL_INDIRECT] Found in element segment"
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }

                                // NO FALLBACK: Per CLAUDE.md, fail loud and early if element not found
                                resolved_func_idx.ok_or_else(|| {
                                    wrt_error::Error::runtime_trap("undefined element")
                                })?
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap("CallIndirect: instance not found"));
                        };

                        #[cfg(feature = "tracing")]
                        trace!(func_idx = func_idx, "[CALL_INDIRECT] Resolved to function index");

                        // CHECK FOR LOWERED FUNCTION: If this function was created by canon.lower,
                        // dispatch to the canonical executor instead of executing bytecode.
                        // This prevents infinite recursion when shim modules have self-referential tables.
                        #[cfg(all(feature = "std", feature = "wasi"))]
                        if self.is_lowered_function(instance_id, func_idx) {
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                func_idx = func_idx,
                                "[CALL_INDIRECT] Function is a canon.lower synthesized function"
                            );

                            // Get function type to determine parameter count
                            let func = &module.functions[func_idx];
                            let func_type = module.types.get(func.type_idx as usize)
                                .ok_or_else(|| wrt_error::Error::runtime_error("Invalid function type"))?;

                            // Pop the required number of arguments from the stack
                            let param_count = func_type.params.len();
                            let mut call_args = Vec::new();
                            for _ in 0..param_count {
                                if let Some(arg) = operand_stack.pop() {
                                    call_args.push(arg);
                                } else {
                                    return Err(wrt_error::Error::runtime_error("Stack underflow on lowered function call"));
                                }
                            }
                            call_args.reverse();

                            // Execute the lowered function via WASI dispatcher
                            let results = self.execute_lowered_function(instance_id, func_idx, call_args)?;

                            // Push results back onto stack
                            for result in results {
                                operand_stack.push(result);
                            }

                            // Skip the normal call_indirect processing
                            pc += 1;
                            continue;
                        }

                        // Track call_indirect to func 138 (iterator next)
                        static CALL_138_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                        #[cfg(all(feature = "std", feature = "tracing"))]
                        if func_idx == 138 {
                            let call_num = CALL_138_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            trace!(
                                call_num = call_num,
                                stack_len = operand_stack.len(),
                                "[CALL-138-INDIRECT] Indirect call to func 138"
                            );
                            for (i, val) in operand_stack.iter().enumerate() {
                                trace!(index = i, value = ?val, "[CALL-138-INDIRECT] Stack value");
                            }
                        }

                        // Validate function index
                        if func_idx >= module.functions.len() {
                            return Err(wrt_error::Error::runtime_trap(
                                "call_indirect: function index out of bounds"
                            ));
                        }

                        // Get function type to determine parameter count
                        let func = &module.functions[func_idx];
                        let func_type = module.types.get(func.type_idx as usize)
                            .ok_or_else(|| wrt_error::Error::runtime_error("Invalid function type"))?;

                        // Validate type matches expected type (structural equivalence)
                        let expected_type = module.types.get(type_idx as usize)
                            .ok_or_else(|| wrt_error::Error::runtime_error("Invalid expected function type"))?;

                        if !func_types_match(expected_type, func_type) {
                            #[cfg(feature = "tracing")]
                            warn!(
                                expected_params = expected_type.params.len(),
                                expected_results = expected_type.results.len(),
                                got_params = func_type.params.len(),
                                got_results = func_type.results.len(),
                                "[CALL_INDIRECT] Type mismatch"
                            );
                            return Err(wrt_error::Error::runtime_trap("indirect call type mismatch"));
                        }

                        // Pop the required number of arguments from the stack
                        let param_count = func_type.params.len();
                        let mut call_args = Vec::new();
                        for _ in 0..param_count {
                            if let Some(arg) = operand_stack.pop() {
                                call_args.push(arg);
                            } else {
                                return Err(wrt_error::Error::runtime_error("Stack underflow on call_indirect"));
                            }
                        }
                        call_args.reverse();

                        // Check if the function is an imported function that's linked to another instance
                        // Imported functions have empty body and locals
                        let is_import = func.body.is_empty() && func.locals.is_empty();

                        if is_import {
                            #[cfg(feature = "tracing")]
                            trace!(
                                func_idx = func_idx,
                                "[CALL_INDIRECT] Target is imported function, checking cross-instance links"
                            );

                            // Try to find the import's module/field name
                            if let Ok((module_name, field_name)) = self.find_import_by_index(&module, func_idx) {
                                #[cfg(feature = "tracing")]
                                trace!(
                                    module_name = %module_name,
                                    field_name = %field_name,
                                    "[CALL_INDIRECT] Resolved import"
                                );

                                // Check if this import is linked to another instance
                                #[cfg(feature = "std")]
                                {
                                    let import_key = (instance_id, module_name.clone(), field_name.clone());
                                    let linked = self.import_links.get(&import_key)
                                        .map(|(ti, en)| (*ti, en.clone()));

                                    if let Some((target_instance, export_name)) = linked {
                                        #[cfg(feature = "tracing")]
                                        trace!(
                                            target_instance = target_instance,
                                            export_name = %export_name,
                                            "[CALL_INDIRECT] Import linked to another instance"
                                        );

                                        // NON-RECURSIVE: resolve target and redirect via trampoline
                                        let target_func = self.resolve_export_func_idx(target_instance, &export_name)?;
                                        let saved_state = SuspendedFrame {
                                            instance_id,
                                            func_idx: caller_func_idx,
                                            pc: pc + 1,
                                            locals,
                                            operand_stack,
                                            block_stack,
                                            block_depth,
                                            instruction_count,
                                        };
                                        return Ok(ExecutionOutcome::Call {
                                            instance_id: target_instance,
                                            func_idx: target_func,
                                            args: call_args,
                                            return_state: Some(saved_state),
                                        });
                                    } else {
                                        // Not linked - dispatch to WASI if applicable
                                        #[cfg(feature = "tracing")]
                                        trace!(
                                            module_name = %module_name,
                                            field_name = %field_name,
                                            "[CALL_INDIRECT] Import not linked, trying WASI dispatch"
                                        );

                                        // Call WASI function (need to push args back for call_wasi_function)
                                        for arg in call_args.iter().rev() {
                                            operand_stack.push(arg.clone());
                                        }
                                        let result = self.call_wasi_function(
                                            &module_name,
                                            &field_name,
                                            &mut operand_stack,
                                            &module,
                                            instance_id,
                                        )?;
                                        if let Some(val) = result {
                                            operand_stack.push(val);
                                        }
                                    }
                                }

                                #[cfg(not(feature = "std"))]
                                {
                                    // NON-RECURSIVE: redirect via trampoline
                                    let saved_state = SuspendedFrame {
                                        instance_id,
                                        func_idx: caller_func_idx,
                                        pc: pc + 1,
                                        locals,
                                        operand_stack,
                                        block_stack,
                                        block_depth,
                                        instruction_count,
                                    };
                                    return Ok(ExecutionOutcome::Call {
                                        instance_id,
                                        func_idx,
                                        args: call_args,
                                        return_state: Some(saved_state),
                                    });
                                }
                            } else {
                                // Couldn't resolve import - redirect via trampoline
                                #[cfg(feature = "tracing")]
                                warn!(
                                    func_idx = func_idx,
                                    "[CALL_INDIRECT] Could not resolve import, executing via trampoline"
                                );
                                let saved_state = SuspendedFrame {
                                    instance_id,
                                    func_idx: caller_func_idx,
                                    pc: pc + 1,
                                    locals,
                                    operand_stack,
                                    block_stack,
                                    block_depth,
                                    instruction_count,
                                };
                                return Ok(ExecutionOutcome::Call {
                                    instance_id,
                                    func_idx,
                                    args: call_args,
                                    return_state: Some(saved_state),
                                });
                            }
                        } else {
                            // Regular function - redirect via trampoline (non-recursive)
                            let saved_state = SuspendedFrame {
                                instance_id,
                                func_idx: caller_func_idx,
                                pc: pc + 1,
                                locals,
                                operand_stack,
                                block_stack,
                                block_depth,
                                instruction_count,
                            };
                            return Ok(ExecutionOutcome::Call {
                                instance_id,
                                func_idx,
                                args: call_args,
                                return_state: Some(saved_state),
                            });
                        }
                    }
                    Instruction::ReturnCall(func_idx) => {
                        // ReturnCall: tail call to another function
                        // Similar to Call, but the results become the current function's return value
                        #[cfg(feature = "tracing")]
                        trace!("⚡ RETURN_CALL: func_idx={}", func_idx);

                        // Get function type to determine parameter count
                        if (func_idx as usize) >= module.functions.len() {
                            return Err(wrt_error::Error::runtime_trap("return_call: function index out of bounds"));
                        }
                        let func = &module.functions[func_idx as usize];
                        let func_type = module.types.get(func.type_idx as usize)
                            .ok_or_else(|| wrt_error::Error::runtime_error("Invalid function type"))?;

                        // Pop the required number of arguments from the stack
                        let param_count = func_type.params.len();
                        let mut call_args = Vec::new();
                        for _ in 0..param_count {
                            if let Some(arg) = operand_stack.pop() {
                                call_args.push(arg);
                            } else {
                                return Err(wrt_error::Error::runtime_error("Stack underflow on return_call"));
                            }
                        }
                        call_args.reverse();

                        // Restore debugger before returning for tail call
                        #[cfg(all(feature = "std", feature = "debugger"))]
                        {
                            self.debugger = debugger_opt;
                        }

                        // Return TailCall - the trampoline will execute the target function
                        // This avoids recursive native calls and prevents stack overflow
                        return Ok(ExecutionOutcome::TailCall {
                            func_idx: func_idx as usize,
                            args: call_args,
                        });
                    }
                    Instruction::ReturnCallIndirect(type_idx, table_idx) => {
                        // ReturnCallIndirect: tail call through indirect table reference
                        // Pop the function index from the stack
                        let table_func_idx = if let Some(Value::I32(idx)) = operand_stack.pop() {
                            idx as u32
                        } else {
                            return Err(wrt_error::Error::runtime_trap("return_call_indirect: expected i32 function index on stack"));
                        };

                        #[cfg(feature = "tracing")]
                        trace!(
                            type_idx = type_idx,
                            table_idx = table_idx,
                            table_func_idx = table_func_idx,
                            "[RETURN_CALL_INDIRECT] Indirect tail call"
                        );

                        // Look up the function in the table
                        let func_idx = if let Some(inst) = self.instances.get(&instance_id) {
                            if let Ok(table) = inst.table(table_idx) {
                                if let Ok(Some(func_ref)) = table.0.get(table_func_idx) {
                                    match func_ref {
                                        Value::FuncRef(Some(fref)) => fref.index as usize,
                                        Value::FuncRef(None) => return Err(wrt_error::Error::runtime_trap("uninitialized element")),
                                        Value::I32(idx) => idx as usize,
                                        Value::I64(idx) => idx as usize,
                                        _ => return Err(wrt_error::Error::runtime_trap("uninitialized element")),
                                    }
                                } else if let Ok(None) = table.0.get(table_func_idx) {
                                    return Err(wrt_error::Error::runtime_trap("uninitialized element"));
                                } else {
                                    return Err(wrt_error::Error::runtime_trap("undefined element"));
                                }
                            } else {
                                return Err(wrt_error::Error::runtime_trap("return_call_indirect: table not found"));
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap("return_call_indirect: instance not found"));
                        };

                        // Validate function index
                        if func_idx >= module.functions.len() {
                            return Err(wrt_error::Error::runtime_trap("return_call_indirect: function index out of bounds"));
                        }

                        // Get function type and validate
                        let func = &module.functions[func_idx];
                        let func_type = module.types.get(func.type_idx as usize)
                            .ok_or_else(|| wrt_error::Error::runtime_error("Invalid function type"))?;

                        let expected_type = module.types.get(type_idx as usize)
                            .ok_or_else(|| wrt_error::Error::runtime_error("Invalid expected function type"))?;

                        if !func_types_match(expected_type, func_type) {
                            return Err(wrt_error::Error::runtime_trap("indirect call type mismatch"));
                        }

                        // Pop the required number of arguments from the stack
                        let param_count = func_type.params.len();
                        let mut call_args = Vec::new();
                        for _ in 0..param_count {
                            if let Some(arg) = operand_stack.pop() {
                                call_args.push(arg);
                            } else {
                                return Err(wrt_error::Error::runtime_error("Stack underflow on return_call_indirect"));
                            }
                        }
                        call_args.reverse();

                        // Restore debugger before returning for tail call
                        #[cfg(all(feature = "std", feature = "debugger"))]
                        {
                            self.debugger = debugger_opt;
                        }

                        // Return TailCall - the trampoline will execute the target function
                        // This avoids recursive native calls and prevents stack overflow
                        return Ok(ExecutionOutcome::TailCall {
                            func_idx,
                            args: call_args,
                        });
                    }
                    Instruction::I32Const(value) => {
                        #[cfg(feature = "tracing")]
                        trace!("I32Const: pushing value {}", value);
                        #[cfg(feature = "tracing")]
                        {
                            if (value as u32) > 0x200000 {
                                warn!(
                                    value = value,
                                    value_hex = format_args!("0x{:x}", value as u32),
                                    "[I32Const] SUSPICIOUS: large value"
                                );
                            }
                        }
                        operand_stack.push(Value::I32(value));
                        #[cfg(feature = "tracing")]
                        trace!("Operand stack now has {} values", operand_stack.len());
                    }
                    Instruction::I64Const(value) => {
                        #[cfg(feature = "tracing")]

                        trace!("I64Const: pushing value {}", value);
                        operand_stack.push(Value::I64(value));
                    }
                    Instruction::F32Const(bits) => {
                        #[cfg(feature = "tracing")]
                        trace!("F32Const: pushing {}", f32::from_bits(bits));
                        operand_stack.push(Value::F32(FloatBits32(bits)));
                    }
                    Instruction::LocalGet(local_idx) => {
                        if (local_idx as usize) < locals.len() {
                            // Use copy_value() for efficient copying of Copy-like variants
                            let value = locals[local_idx as usize].copy_value();
                            #[cfg(feature = "tracing")]
                            trace!("LocalGet: local[{}] = {:?}", local_idx, value);
                            // Debug: trace LocalGet in function 222 for key locals (0=pieces, 2=Arguments)
                            #[cfg(feature = "tracing")]
                            if func_idx == 222 && (local_idx == 0 || local_idx == 2) {
                                trace!(
                                    pc = pc,
                                    local_idx = local_idx,
                                    value = ?value,
                                    "[LOCALGET-222] Key local access"
                                );
                            }
                            #[cfg(feature = "tracing")]
                            {
                                // Debug suspicious values that might be bad pointers
                                if let Value::I32(v) = &value {
                                    if (*v as u32) > 0x200000 {
                                        warn!(
                                            local_idx = local_idx,
                                            value = v,
                                            value_hex = format_args!("0x{:x}", *v as u32),
                                            "[LocalGet] SUSPICIOUS: large value"
                                        );
                                    }
                                }
                            }
                            operand_stack.push(value);
                            #[cfg(feature = "tracing")]
                            trace!("Operand stack now has {} values", operand_stack.len());
                        } else {
                            #[cfg(feature = "tracing")]
                            trace!("LocalGet: local[{}] out of bounds (locals.len()={})", local_idx, locals.len());
                        }
                    }
                    Instruction::LocalSet(local_idx) => {
                        if let Some(value) = operand_stack.pop() {
                            #[cfg(feature = "tracing")]
                            trace!("LocalSet: setting local[{}] = {:?}", local_idx, value);
                            // Debug: trace LocalSet in function 222 (core::fmt::write)
                            #[cfg(feature = "tracing")]
                            if func_idx == 222 {
                                trace!(
                                    pc = pc,
                                    local_idx = local_idx,
                                    value = ?value,
                                    "[LOCALSET-222] Local set"
                                );
                            }
                            if (local_idx as usize) < locals.len() {
                                locals[local_idx as usize] = value;
                            } else {
                                #[cfg(feature = "tracing")]

                                trace!("LocalSet: local[{}] out of bounds (locals.len()={})", local_idx, locals.len());
                            }
                        } else {
                            #[cfg(feature = "tracing")]

                            trace!("LocalSet: operand stack empty");
                        }
                    }
                    Instruction::LocalTee(local_idx) => {
                        // Like LocalSet but keeps value on stack
                        // Use copy_value() for efficient copying of Copy-like variants
                        if let Some(value) = operand_stack.last().map(|v| v.copy_value()) {
                            #[cfg(feature = "tracing")]

                            trace!("LocalTee: setting local[{}] = {:?} (keeping on stack)", local_idx, value);
                            // Debug: trace LocalTee in function 222 (core::fmt::write)
                            #[cfg(feature = "tracing")]
                            if func_idx == 222 {
                                trace!(
                                    pc = pc,
                                    local_idx = local_idx,
                                    value = ?value,
                                    "[LOCALTEE-222] Local tee"
                                );
                            }
                            if (local_idx as usize) < locals.len() {
                                locals[local_idx as usize] = value;
                            } else {
                                #[cfg(feature = "tracing")]

                                trace!("LocalTee: local[{}] out of bounds (locals.len()={})", local_idx, locals.len());
                            }
                        } else {
                            #[cfg(feature = "tracing")]

                            trace!("LocalTee: operand stack empty");
                        }
                    }
                    Instruction::GlobalGet(global_idx) => {
                        #[cfg(feature = "tracing")]
                        trace!("GlobalGet: reading global[{}] from instance", global_idx);

                        match instance.global(global_idx) {
                            Ok(global_wrapper) => {
                                // GlobalWrapper now uses Arc<RwLock<Global>>, use get() method
                                let value = global_wrapper.get().map_err(|_| {
                                    wrt_error::Error::runtime_execution_error(
                                        "Failed to read global value"
                                    )
                                })?;
                                #[cfg(feature = "tracing")]
                                trace!("GlobalGet: global[{}] = {:?} (from instance)", global_idx, value);
                                #[cfg(feature = "tracing")]
                                {
                                    // Debug all global reads to trace suspicious values
                                    match &value {
                                        Value::I32(v) => trace!(
                                            global_idx = global_idx,
                                            value = v,
                                            value_hex = format_args!("0x{:x}", *v as u32),
                                            "[GlobalGet] Global value"
                                        ),
                                        _ => trace!(
                                            global_idx = global_idx,
                                            value = ?value,
                                            "[GlobalGet] Global value"
                                        ),
                                    }
                                }
                                operand_stack.push(value);
                            }
                            Err(e) => {
                                #[cfg(feature = "tracing")]
                                error!("GlobalGet: failed to get global[{}]: {:?}", global_idx, e);
                                // NO FALLBACKS - fail properly as per user directive
                                return Err(wrt_error::Error::runtime_execution_error(
                                    "Failed to get global from instance"
                                ));
                            }
                        }
                    }
                    Instruction::GlobalSet(global_idx) => {

                        if let Some(value) = operand_stack.pop() {
                            #[cfg(feature = "tracing")]
                            trace!("GlobalSet: setting global[{}] to {:?}", global_idx, value);
                            #[cfg(feature = "tracing")]
                            {
                                match &value {
                                    Value::I32(v) => trace!(
                                        global_idx = global_idx,
                                        value = v,
                                        value_hex = format_args!("0x{:x}", *v as u32),
                                        "[GlobalSet] Setting global"
                                    ),
                                    _ => trace!(
                                        global_idx = global_idx,
                                        value = ?value,
                                        "[GlobalSet] Setting global"
                                    ),
                                }
                            }

                            // GlobalWrapper now uses Arc<RwLock<Global>> for interior mutability
                            match instance.global(global_idx) {
                                Ok(global_wrapper) => {
                                    global_wrapper.set(value).map_err(|e| {
                                        wrt_error::Error::runtime_execution_error(
                                            "GlobalSet: failed to set global value"
                                        )
                                    })?;
                                    #[cfg(feature = "tracing")]
                                    trace!("GlobalSet: successfully set global[{}]", global_idx);
                                }
                                Err(_e) => {
                                    return Err(wrt_error::Error::runtime_execution_error(
                                        "GlobalSet: global index out of bounds"
                                    ));
                                }
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_execution_error(
                                "GlobalSet requires a value on the operand stack"
                            ));
                        }
                    }
                    // Arithmetic operations
                    Instruction::I32Add => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_add(b);
                            #[cfg(feature = "tracing")]
                            trace!("I32Add: {} + {} = {}", a, b, result);
                            #[cfg(feature = "tracing")]
                            {
                                if (result as u32) > 0x200000 {
                                    warn!(
                                        a = a,
                                        b = b,
                                        result = result,
                                        result_hex = format_args!("0x{:x}", result as u32),
                                        "[I32Add] SUSPICIOUS: large result"
                                    );
                                }
                            }
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Sub => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_sub(b);
                            #[cfg(feature = "tracing")]
                            trace!("I32Sub: {} - {} = {}", a, b, result);
                            #[cfg(feature = "tracing")]
                            {
                                if (result as u32) > 0x200000 {
                                    warn!(
                                        a = a,
                                        b = b,
                                        result = result,
                                        result_hex = format_args!("0x{:x}", result as u32),
                                        "[I32Sub] SUSPICIOUS: large result"
                                    );
                                }
                            }
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Mul => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_mul(b);
                            #[cfg(feature = "tracing")]
                            trace!("I32Mul: {} * {} = {}", a, b, result);
                            #[cfg(feature = "tracing")]
                            {
                                if (result as u32) > 0x20000 {
                                    warn!(
                                        a = a,
                                        b = b,
                                        result = result,
                                        result_hex = format_args!("0x{:x}", result as u32),
                                        "[I32Mul] SUSPICIOUS: large result"
                                    );
                                }
                            }
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32DivS => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            if b == 0 {
                                return Err(wrt_error::Error::runtime_trap("integer divide by zero"));
                            }
                            // Check for integer overflow: INT_MIN / -1 would overflow
                            if a == i32::MIN && b == -1 {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let result = a.wrapping_div(b);
                            #[cfg(feature = "tracing")]
                            trace!("I32DivS: {} / {} = {}", a, b, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32DivU => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            if b == 0 {
                                return Err(wrt_error::Error::runtime_trap("integer divide by zero"));
                            }
                            let result = (a as u32).wrapping_div(b as u32) as i32;
                            #[cfg(feature = "tracing")]
                            trace!("I32DivU: {} / {} = {}", a as u32, b as u32, result as u32);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32RemS => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            if b == 0 {
                                return Err(wrt_error::Error::runtime_trap("integer divide by zero"));
                            }
                            // Note: INT_MIN % -1 = 0 (no overflow for remainder)
                            let result = a.wrapping_rem(b);
                            #[cfg(feature = "tracing")]
                            trace!("I32RemS: {} % {} = {}", a, b, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32RemU => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            if b == 0 {
                                return Err(wrt_error::Error::runtime_trap("integer divide by zero"));
                            }
                            let result = (a as u32).wrapping_rem(b as u32) as i32;
                            #[cfg(feature = "tracing")]
                            trace!("I32RemU: {} % {} = {}", a as u32, b as u32, result as u32);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    // I64 arithmetic operations
                    Instruction::I64Add => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_add(b);
                            #[cfg(feature = "tracing")]

                            trace!("I64Add: {} + {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Sub => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_sub(b);
                            #[cfg(feature = "tracing")]

                            trace!("I64Sub: {} - {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Mul => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_mul(b);
                            #[cfg(feature = "tracing")]

                            trace!("I64Mul: {} * {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    // I64 division and remainder operations
                    Instruction::I64DivS => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            if b == 0 {
                                return Err(wrt_error::Error::runtime_trap("integer divide by zero"));
                            }
                            // Check for integer overflow: INT_MIN / -1 would overflow
                            if a == i64::MIN && b == -1 {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let result = a.wrapping_div(b);
                            #[cfg(feature = "tracing")]
                            trace!("I64DivS: {} / {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64DivU => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            if b == 0 {
                                return Err(wrt_error::Error::runtime_trap("integer divide by zero"));
                            }
                            let result = (a as u64).wrapping_div(b as u64) as i64;
                            #[cfg(feature = "tracing")]
                            trace!("I64DivU: {} / {} = {}", a as u64, b as u64, result as u64);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64RemS => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            if b == 0 {
                                return Err(wrt_error::Error::runtime_trap("integer divide by zero"));
                            }
                            // Note: INT_MIN % -1 = 0 (no overflow for remainder)
                            let result = a.wrapping_rem(b);
                            #[cfg(feature = "tracing")]
                            trace!("I64RemS: {} % {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64RemU => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            if b == 0 {
                                return Err(wrt_error::Error::runtime_trap("integer divide by zero"));
                            }
                            let result = (a as u64).wrapping_rem(b as u64) as i64;
                            #[cfg(feature = "tracing")]
                            trace!("I64RemU: {} % {} = {}", a as u64, b as u64, result as u64);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    // I64 shift and rotate operations
                    Instruction::I64Shl => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_shl((b as u64 % 64) as u32);
                            #[cfg(feature = "tracing")]
                            trace!("I64Shl: {} << {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64ShrS => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_shr((b as u64 % 64) as u32);
                            #[cfg(feature = "tracing")]
                            trace!("I64ShrS: {} >> {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64ShrU => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = (a as u64).wrapping_shr((b as u64 % 64) as u32) as i64;
                            #[cfg(feature = "tracing")]
                            trace!("I64ShrU: {} >> {} = {}", a as u64, b as u64, result as u64);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Rotl => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.rotate_left((b as u64 % 64) as u32);
                            #[cfg(feature = "tracing")]
                            trace!("I64Rotl: {} rotl {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Rotr => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.rotate_right((b as u64 % 64) as u32);
                            #[cfg(feature = "tracing")]
                            trace!("I64Rotr: {} rotr {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    // Conversion operations
                    Instruction::I32WrapI64 => {
                        if let Some(Value::I64(value)) = operand_stack.pop() {
                            let result = value as i32;
                            #[cfg(feature = "tracing")]
                            trace!("I32WrapI64: {} -> {}", value, result);
                            #[cfg(feature = "tracing")]
                            {
                                if (result as u32) > 0x20000 {
                                    warn!(
                                        i64_value = value,
                                        i64_hex = format_args!("0x{:x}", value),
                                        i32_result = result,
                                        i32_hex = format_args!("0x{:x}", result as u32),
                                        "[I32WrapI64] SUSPICIOUS: large wrap result"
                                    );
                                }
                            }
                            operand_stack.push(Value::I32(result));
                        }
                    }

                    // Trapping truncation operations - trap on NaN or overflow
                    // Per WebAssembly spec: NaN -> "invalid conversion to integer"
                    //                       Infinity or out-of-range -> "integer overflow"
                    Instruction::I32TruncF32S => {
                        if let Some(Value::F32(bits)) = operand_stack.pop() {
                            let f = f32::from_bits(bits.0);
                            if f.is_nan() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "invalid conversion to integer",
                                ));
                            }
                            if f.is_infinite() {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let f_trunc = f.trunc();
                            // Range check: must be in [-2147483648, 2147483647]
                            if f_trunc < -2_147_483_648.0_f32 || f_trunc >= 2_147_483_648.0_f32 {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            operand_stack.push(Value::I32(f_trunc as i32));
                        }
                    }
                    Instruction::I32TruncF32U => {
                        if let Some(Value::F32(bits)) = operand_stack.pop() {
                            let f = f32::from_bits(bits.0);
                            if f.is_nan() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "invalid conversion to integer",
                                ));
                            }
                            if f.is_infinite() {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let f_trunc = f.trunc();
                            // Range check: must be in [0, 4294967295]
                            if f_trunc < 0.0_f32 || f_trunc >= 4_294_967_296.0_f32 {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            operand_stack.push(Value::I32(f_trunc as u32 as i32));
                        }
                    }
                    Instruction::I32TruncF64S => {
                        if let Some(Value::F64(bits)) = operand_stack.pop() {
                            let f = f64::from_bits(bits.0);
                            if f.is_nan() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "invalid conversion to integer",
                                ));
                            }
                            if f.is_infinite() {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let f_trunc = f.trunc();
                            // Range check: must be in [-2147483648, 2147483647]
                            if f_trunc < -2_147_483_648.0_f64 || f_trunc >= 2_147_483_648.0_f64 {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            operand_stack.push(Value::I32(f_trunc as i32));
                        }
                    }
                    Instruction::I32TruncF64U => {
                        if let Some(Value::F64(bits)) = operand_stack.pop() {
                            let f = f64::from_bits(bits.0);
                            if f.is_nan() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "invalid conversion to integer",
                                ));
                            }
                            if f.is_infinite() {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let f_trunc = f.trunc();
                            // Range check: must be in [0, 4294967295]
                            if f_trunc < 0.0_f64 || f_trunc >= 4_294_967_296.0_f64 {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            operand_stack.push(Value::I32(f_trunc as u32 as i32));
                        }
                    }
                    Instruction::I64TruncF32S => {
                        if let Some(Value::F32(bits)) = operand_stack.pop() {
                            let f = f32::from_bits(bits.0);
                            if f.is_nan() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "invalid conversion to integer",
                                ));
                            }
                            if f.is_infinite() {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let f_trunc = f.trunc();
                            // Range check: must be in [-9223372036854775808, 9223372036854775807]
                            if f_trunc < -9_223_372_036_854_775_808.0_f32
                                || f_trunc >= 9_223_372_036_854_775_808.0_f32
                            {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            operand_stack.push(Value::I64(f_trunc as i64));
                        }
                    }
                    Instruction::I64TruncF32U => {
                        if let Some(Value::F32(bits)) = operand_stack.pop() {
                            let f = f32::from_bits(bits.0);
                            if f.is_nan() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "invalid conversion to integer",
                                ));
                            }
                            if f.is_infinite() {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let f_trunc = f.trunc();
                            // Range check: must be in [0, 18446744073709551615]
                            if f_trunc < 0.0_f32 || f_trunc >= 18_446_744_073_709_551_616.0_f32 {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            operand_stack.push(Value::I64(f_trunc as u64 as i64));
                        }
                    }
                    Instruction::I64TruncF64S => {
                        if let Some(Value::F64(bits)) = operand_stack.pop() {
                            let f = f64::from_bits(bits.0);
                            if f.is_nan() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "invalid conversion to integer",
                                ));
                            }
                            if f.is_infinite() {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let f_trunc = f.trunc();
                            // Range check: must be in [-9223372036854775808, 9223372036854775807]
                            if f_trunc < -9_223_372_036_854_775_808.0_f64
                                || f_trunc >= 9_223_372_036_854_775_808.0_f64
                            {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            operand_stack.push(Value::I64(f_trunc as i64));
                        }
                    }
                    Instruction::I64TruncF64U => {
                        if let Some(Value::F64(bits)) = operand_stack.pop() {
                            let f = f64::from_bits(bits.0);
                            if f.is_nan() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "invalid conversion to integer",
                                ));
                            }
                            if f.is_infinite() {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            let f_trunc = f.trunc();
                            // Range check: must be in [0, 18446744073709551615]
                            if f_trunc < 0.0_f64 || f_trunc >= 18_446_744_073_709_551_616.0_f64 {
                                return Err(wrt_error::Error::runtime_trap("integer overflow"));
                            }
                            operand_stack.push(Value::I64(f_trunc as u64 as i64));
                        }
                    }

                    // Saturating truncation operations - clamp instead of trap on overflow
                    Instruction::I32TruncSatF32S => {
                        if let Some(value) = operand_stack.pop() {
                            let result = match value {
                                Value::F32(bits) => {
                                    let f = f32::from_bits(bits.0);
                                    if f.is_nan() {
                                        0i32
                                    } else if f >= i32::MAX as f32 {
                                        i32::MAX
                                    } else if f <= i32::MIN as f32 {
                                        i32::MIN
                                    } else {
                                        f as i32
                                    }
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]

                                    trace!("I32TruncSatF32S: type error");
                                    0i32
                                }
                            };
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32TruncSatF32U => {
                        if let Some(value) = operand_stack.pop() {
                            let result = match value {
                                Value::F32(bits) => {
                                    let f = f32::from_bits(bits.0);
                                    if f.is_nan() || f <= 0.0 {
                                        0u32
                                    } else if f >= u32::MAX as f32 {
                                        u32::MAX
                                    } else {
                                        f as u32
                                    }
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]

                                    trace!("I32TruncSatF32U: type error");
                                    0u32
                                }
                            };
                            operand_stack.push(Value::I32(result as i32));
                        }
                    }
                    Instruction::I32TruncSatF64S => {
                        if let Some(value) = operand_stack.pop() {
                            let result = match value {
                                Value::F64(bits) => {
                                    let f = f64::from_bits(bits.0);
                                    if f.is_nan() {
                                        0i32
                                    } else if f >= i32::MAX as f64 {
                                        i32::MAX
                                    } else if f <= i32::MIN as f64 {
                                        i32::MIN
                                    } else {
                                        f as i32
                                    }
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]

                                    trace!("I32TruncSatF64S: type error");
                                    0i32
                                }
                            };
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32TruncSatF64U => {
                        if let Some(value) = operand_stack.pop() {
                            let result = match value {
                                Value::F64(bits) => {
                                    let f = f64::from_bits(bits.0);
                                    if f.is_nan() || f <= 0.0 {
                                        0u32
                                    } else if f >= u32::MAX as f64 {
                                        u32::MAX
                                    } else {
                                        f as u32
                                    }
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]

                                    trace!("I32TruncSatF64U: type error");
                                    0u32
                                }
                            };
                            operand_stack.push(Value::I32(result as i32));
                        }
                    }
                    Instruction::I64TruncSatF32S => {
                        if let Some(value) = operand_stack.pop() {
                            let result = match value {
                                Value::F32(bits) => {
                                    let f = f32::from_bits(bits.0);
                                    if f.is_nan() {
                                        0i64
                                    } else if f >= i64::MAX as f32 {
                                        i64::MAX
                                    } else if f <= i64::MIN as f32 {
                                        i64::MIN
                                    } else {
                                        f as i64
                                    }
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]

                                    trace!("I64TruncSatF32S: type error");
                                    0i64
                                }
                            };
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64TruncSatF32U => {
                        if let Some(value) = operand_stack.pop() {
                            let result = match value {
                                Value::F32(bits) => {
                                    let f = f32::from_bits(bits.0);
                                    if f.is_nan() || f <= 0.0 {
                                        0u64
                                    } else if f >= u64::MAX as f32 {
                                        u64::MAX
                                    } else {
                                        f as u64
                                    }
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]

                                    trace!("I64TruncSatF32U: type error");
                                    0u64
                                }
                            };
                            operand_stack.push(Value::I64(result as i64));
                        }
                    }
                    Instruction::I64TruncSatF64S => {
                        if let Some(value) = operand_stack.pop() {
                            let result = match value {
                                Value::F64(bits) => {
                                    let f = f64::from_bits(bits.0);
                                    if f.is_nan() {
                                        0i64
                                    } else if f >= i64::MAX as f64 {
                                        i64::MAX
                                    } else if f <= i64::MIN as f64 {
                                        i64::MIN
                                    } else {
                                        f as i64
                                    }
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]

                                    trace!("I64TruncSatF64S: type error");
                                    0i64
                                }
                            };
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64TruncSatF64U => {
                        if let Some(value) = operand_stack.pop() {
                            let result = match value {
                                Value::F64(bits) => {
                                    let f = f64::from_bits(bits.0);
                                    if f.is_nan() || f <= 0.0 {
                                        0u64
                                    } else if f >= u64::MAX as f64 {
                                        u64::MAX
                                    } else {
                                        f as u64
                                    }
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]

                                    trace!("I64TruncSatF64U: type error");
                                    0u64
                                }
                            };
                            operand_stack.push(Value::I64(result as i64));
                        }
                    }

                    // Comparison operations
                    Instruction::I32Eq => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a == b { 1 } else { 0 };
                            #[cfg(feature = "tracing")]
                            trace!("I32Eq: {} == {} = {}", a, b, result != 0);
                            // Trace None checks (comparing to -2147483648 sentinel)
                            #[cfg(feature = "tracing")]
                            if b == -2147483648 || a == -2147483648 {
                                trace!(
                                    func_idx = func_idx,
                                    pc = pc,
                                    a = a,
                                    b = b,
                                    result = result != 0,
                                    "[NONE_CHECK] None sentinel check"
                                );
                            }
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Ne => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a != b { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32Ne: {} != {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32LtS => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a < b { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32LtS: {} < {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32LtU => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if (a as u32) < (b as u32) { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32LtU: {} < {} = {}", a as u32, b as u32, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32GtS => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a > b { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32GtS: {} > {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32GtU => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if (a as u32) > (b as u32) { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32GtU: {} > {} = {}", a as u32, b as u32, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32LeS => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a <= b { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32LeS: {} <= {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32LeU => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if (a as u32) <= (b as u32) { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32LeU: {} <= {} = {}", a as u32, b as u32, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32GeS => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a >= b { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32GeS: {} >= {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32GeU => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if (a as u32) >= (b as u32) { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32GeU: {} >= {} = {}", a as u32, b as u32, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    // I64 comparison operations - all produce i32 result
                    Instruction::I64Eq => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a == b { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64Eq: {} == {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64Ne => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a != b { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64Ne: {} != {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64LtS => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a < b { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64LtS: {} < {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64LtU => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if (a as u64) < (b as u64) { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64LtU: {} < {} = {}", a as u64, b as u64, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64GtS => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a > b { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64GtS: {} > {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64GtU => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if (a as u64) > (b as u64) { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64GtU: {} > {} = {}", a as u64, b as u64, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64LeS => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a <= b { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64LeS: {} <= {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64LeU => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if (a as u64) <= (b as u64) { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64LeU: {} <= {} = {}", a as u64, b as u64, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64GeS => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a >= b { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64GeS: {} >= {} = {}", a, b, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64GeU => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if (a as u64) >= (b as u64) { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64GeU: {} >= {} = {}", a as u64, b as u64, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64Eqz => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = if a == 0 { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("I64Eqz: {} == 0 = {}", a, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    // F32 comparison operations - all produce i32 result
                    Instruction::F32Eq => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a.value() == b.value() { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("F32Eq: {} == {} = {}", a.value(), b.value(), result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F32Ne => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a.value() != b.value() { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("F32Ne: {} != {} = {}", a.value(), b.value(), result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F32Lt => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a.value() < b.value() { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("F32Lt: {} < {} = {}", a.value(), b.value(), result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F32Gt => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a.value() > b.value() { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("F32Gt: {} > {} = {}", a.value(), b.value(), result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F32Le => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a.value() <= b.value() { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("F32Le: {} <= {} = {}", a.value(), b.value(), result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F32Ge => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if a.value() >= b.value() { 1i32 } else { 0i32 };
                            #[cfg(feature = "tracing")]
                            trace!("F32Ge: {} >= {} = {}", a.value(), b.value(), result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    // Bitwise operations
                    Instruction::I32And => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a & b;
                            #[cfg(feature = "tracing")]

                            trace!("I32And: {} & {} = {}", a, b, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Or => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a | b;
                            #[cfg(feature = "tracing")]

                            trace!("I32Or: {} | {} = {}", a, b, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Xor => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a ^ b;
                            #[cfg(feature = "tracing")]
                            trace!("I32Xor: {} ^ {} = {}", a, b, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    // I64 bitwise operations
                    Instruction::I64And => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a & b;
                            #[cfg(feature = "tracing")]
                            trace!("I64And: {} & {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Or => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a | b;
                            #[cfg(feature = "tracing")]
                            trace!("I64Or: {} | {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Xor => {
                        if let (Some(Value::I64(b)), Some(Value::I64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a ^ b;
                            #[cfg(feature = "tracing")]
                            trace!("I64Xor: {} ^ {} = {}", a, b, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I32Shl => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_shl((b as u32) % 32);
                            #[cfg(feature = "tracing")]
                            trace!("I32Shl: {} << {} = {}", a, b, result);
                            #[cfg(feature = "tracing")]
                            {
                                if (result as u32) > 0x200000 {
                                    warn!(
                                        a = a,
                                        b = b,
                                        result = result,
                                        result_hex = format_args!("0x{:x}", result as u32),
                                        "[I32Shl] SUSPICIOUS: large shift result"
                                    );
                                }
                            }
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32ShrS => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.wrapping_shr((b as u32) % 32);
                            #[cfg(feature = "tracing")]

                            trace!("I32ShrS: {} >> {} = {}", a, b, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32ShrU => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = (a as u32).wrapping_shr((b as u32) % 32) as i32;
                            #[cfg(feature = "tracing")]

                            trace!("I32ShrU: {} >> {} = {}", a as u32, b as u32, result as u32);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Rotl => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.rotate_left((b as u32) % 32);
                            #[cfg(feature = "tracing")]

                            trace!("I32Rotl: {} rotl {} = {}", a, b, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Rotr => {
                        if let (Some(Value::I32(b)), Some(Value::I32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.rotate_right((b as u32) % 32);
                            #[cfg(feature = "tracing")]

                            trace!("I32Rotr: {} rotr {} = {}", a, b, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    // Unary operations
                    Instruction::I32Clz => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = a.leading_zeros() as i32;
                            #[cfg(feature = "tracing")]

                            trace!("I32Clz: clz({}) = {}", a, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Ctz => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = a.trailing_zeros() as i32;
                            #[cfg(feature = "tracing")]

                            trace!("I32Ctz: ctz({}) = {}", a, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Popcnt => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = a.count_ones() as i32;
                            #[cfg(feature = "tracing")]
                            trace!("I32Popcnt: popcnt({}) = {}", a, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    // I64 unary bit operations
                    Instruction::I64Clz => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = (a as u64).leading_zeros() as i64;
                            #[cfg(feature = "tracing")]
                            trace!("I64Clz: clz({}) = {}", a, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Ctz => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = (a as u64).trailing_zeros() as i64;
                            #[cfg(feature = "tracing")]
                            trace!("I64Ctz: ctz({}) = {}", a, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Popcnt => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = (a as u64).count_ones() as i64;
                            #[cfg(feature = "tracing")]
                            trace!("I64Popcnt: popcnt({}) = {}", a, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I32Eqz => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = if a == 0 { 1 } else { 0 };
                            #[cfg(feature = "tracing")]

                            trace!("I32Eqz: {} == 0 = {}", a, result != 0);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64ExtendI32S => {
                        // Extend i32 to i64 with sign extension
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = a as i64;  // Sign-extends automatically
                            #[cfg(feature = "tracing")]

                            trace!("I64ExtendI32S: {} -> {}", a, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64ExtendI32U => {
                        // Extend i32 to i64 with zero extension
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = (a as u32) as i64;  // Zero-extends
                            #[cfg(feature = "tracing")]

                            trace!("I64ExtendI32U: {} -> {}", a, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    // Sign-extension operators (WebAssembly 2.0)
                    Instruction::I32Extend8S => {
                        // Sign-extend 8-bit value to 32 bits
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = (a as i8) as i32; // Cast to i8 then sign-extend to i32
                            #[cfg(feature = "tracing")]
                            trace!("I32Extend8S: {} -> {}", a, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I32Extend16S => {
                        // Sign-extend 16-bit value to 32 bits
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = (a as i16) as i32; // Cast to i16 then sign-extend to i32
                            #[cfg(feature = "tracing")]
                            trace!("I32Extend16S: {} -> {}", a, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::I64Extend8S => {
                        // Sign-extend 8-bit value to 64 bits
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = (a as i8) as i64; // Cast to i8 then sign-extend to i64
                            #[cfg(feature = "tracing")]
                            trace!("I64Extend8S: {} -> {}", a, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Extend16S => {
                        // Sign-extend 16-bit value to 64 bits
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = (a as i16) as i64; // Cast to i16 then sign-extend to i64
                            #[cfg(feature = "tracing")]
                            trace!("I64Extend16S: {} -> {}", a, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    Instruction::I64Extend32S => {
                        // Sign-extend 32-bit value to 64 bits
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = (a as i32) as i64; // Cast to i32 then sign-extend to i64
                            #[cfg(feature = "tracing")]
                            trace!("I64Extend32S: {} -> {}", a, result);
                            operand_stack.push(Value::I64(result));
                        }
                    }
                    // Memory operations
                    // IMPORTANT: Use instance.memory() for initialized memory, not module.get_memory()
                    // The instance has data segments applied, the module is just a template
                    Instruction::I32Load(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            // Calculate effective address with overflow checking (4 bytes for i32)
                            let offset = calculate_effective_address(addr, mem_arg.offset, 4)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!("I32Load: reading from address {} (base={}, offset={})", offset, addr, mem_arg.offset);
                            // Get memory from INSTANCE (not module) - instance has initialized data
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = i32::from_le_bytes(buffer);
                                            #[cfg(feature = "tracing")]
                                            trace!("I32Load: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I32(value));
                                        }
                                        Err(e) => {
                                            #[cfg(feature = "tracing")]
                                            trace!("I32Load: memory read failed: {:?}", e);
                                            #[cfg(feature = "tracing")]
                                            error!(
                                                offset = format_args!("0x{:x}", offset),
                                                base = format_args!("0x{:x}", addr as u32),
                                                mem_arg_offset = mem_arg.offset,
                                                func_idx = func_idx,
                                                pc = pc,
                                                "[MEM-OOB] I32Load failed"
                                            );
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(e) => {
                                    #[cfg(feature = "tracing")]
                                    trace!("I32Load: failed to get memory at index {}: {:?}", mem_arg.memory_index, e);
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I32Store(mem_arg) => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            // Calculate effective address with overflow checking (4 bytes for i32)
                            let offset = calculate_effective_address(addr, mem_arg.offset, 4)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!("I32Store: writing value {} to address {} (base={}, offset={})", value, offset, addr, mem_arg.offset);

                            // Get memory from INSTANCE (not module) - instance has initialized data
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = value.to_le_bytes();
                                    // ASIL-B COMPLIANT: Use write_shared for thread-safe writes
                                    match memory.write_shared(offset, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!("I32Store: successfully wrote value {} to address {}", value, offset);
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I32Load8S(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 1)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!("I32Load8S: reading from address {}", offset);
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = buffer[0] as i8 as i32; // Sign extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I32Load8S: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I32(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I32Load8U(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 1)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = buffer[0] as i32; // Zero extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I32Load8U: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I32(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I32Load16S(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 2)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = i16::from_le_bytes(buffer) as i32; // Sign extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I32Load16S: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I32(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I32Load16U(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 2)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = u16::from_le_bytes(buffer) as i32; // Zero extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I32Load16U: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I32(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I32Store8(mem_arg) => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 1)? as u32;

                            #[cfg(feature = "tracing")]
                            trace!("I32Store8: writing byte {} to address {}", value & 0xFF, offset);

                            match instance.memory(mem_arg.memory_index) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = [(value & 0xFF) as u8];
                                    match memory.write_shared(offset, &bytes) {
                                        Ok(()) => {}
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I32Store16(mem_arg) => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 2)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = (value as u16).to_le_bytes();
                                    match memory.write_shared(offset, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!("I32Store16: successfully wrote value {} to address {}", value as u16, offset);
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Load(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 8)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = i64::from_le_bytes(buffer);
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Load: read value {} from address {}", value, offset);
                                            #[cfg(all(feature = "std", feature = "tracing"))]
                                            {
                                                trace!(
                                                    value = value,
                                                    value_hex = format_args!("0x{:x}", value as u64),
                                                    offset = format_args!("{:#x}", offset),
                                                    "[I64Load] 64-bit load"
                                                );
                                                // Debug: show memory identity for datetime location
                                                if offset >= 0xffe40 && offset <= 0xffe50 {
                                                    // Get pointer to the actual Memory data via Arc::as_ptr
                                                    use std::sync::Arc;
                                                    let arc_ptr = Arc::as_ptr(&memory_wrapper.0);
                                                    trace!(
                                                        instance_id = instance_id,
                                                        memory_arc_ptr = format_args!("{:p}", arc_ptr),
                                                        "[I64Load-DEBUG] Datetime memory access"
                                                    );
                                                }
                                            }
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Store(mem_arg) => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 8)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = value.to_le_bytes();
                                    // ASIL-B COMPLIANT: Use write_shared for thread-safe writes
                                    match memory.write_shared(offset, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Store: successfully wrote value {} to address {}", value, offset);
                                        }
                                        Err(_e) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_e) => {
                                    #[cfg(feature = "tracing")]
                                    trace!("I64Store: failed to get memory at index {}", mem_arg.memory_index);
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    // ========================================
                    // I64 Partial Load Instructions (load narrower value, extend to i64)
                    // ========================================
                    Instruction::I64Load8S(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 1)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                "[I64Load8S] Signed byte load to i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = buffer[0] as i8 as i64; // Sign extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Load8S: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Load8U(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 1)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                "[I64Load8U] Unsigned byte load to i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = buffer[0] as i64; // Zero extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Load8U: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Load16S(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 2)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                "[I64Load16S] Signed 16-bit load to i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = i16::from_le_bytes(buffer) as i64; // Sign extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Load16S: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Load16U(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 2)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                "[I64Load16U] Unsigned 16-bit load to i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = u16::from_le_bytes(buffer) as i64; // Zero extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Load16U: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Load32S(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 4)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                "[I64Load32S] Signed 32-bit load to i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = i32::from_le_bytes(buffer) as i64; // Sign extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Load32S: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Load32U(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 4)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                "[I64Load32U] Unsigned 32-bit load to i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let value = u32::from_le_bytes(buffer) as i64; // Zero extend
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Load32U: read value {} from address {}", value, offset);
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    // ========================================
                    // I64 Partial Store Instructions (store lower bits of i64)
                    // ========================================
                    Instruction::I64Store8(mem_arg) => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 1)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                value = value & 0xFF,
                                "[I64Store8] Store low 8 bits of i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = [(value & 0xFF) as u8];
                                    // ASIL-B COMPLIANT: Use write_shared for thread-safe writes
                                    match memory.write_shared(offset, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Store8: successfully wrote value {} to address {}", value & 0xFF, offset);
                                        }
                                        Err(_e) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_e) => {
                                    #[cfg(feature = "tracing")]
                                    trace!("I64Store8: failed to get memory at index {}", mem_arg.memory_index);
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Store16(mem_arg) => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 2)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                value = value & 0xFFFF,
                                "[I64Store16] Store low 16 bits of i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = (value as u16).to_le_bytes();
                                    // ASIL-B COMPLIANT: Use write_shared for thread-safe writes
                                    match memory.write_shared(offset, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Store16: successfully wrote value {} to address {}", value & 0xFFFF, offset);
                                        }
                                        Err(_e) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_e) => {
                                    #[cfg(feature = "tracing")]
                                    trace!("I64Store16: failed to get memory at index {}", mem_arg.memory_index);
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::I64Store32(mem_arg) => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 4)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!(
                                instance_id = instance_id,
                                addr = addr,
                                offset = format_args!("{:#x}", offset),
                                value = value & 0xFFFFFFFF,
                                "[I64Store32] Store low 32 bits of i64"
                            );
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = (value as u32).to_le_bytes();
                                    // ASIL-B COMPLIANT: Use write_shared for thread-safe writes
                                    match memory.write_shared(offset, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!("I64Store32: successfully wrote value {} to address {}", value & 0xFFFFFFFF, offset);
                                        }
                                        Err(_e) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_e) => {
                                    #[cfg(feature = "tracing")]
                                    trace!("I64Store32: failed to get memory at index {}", mem_arg.memory_index);
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    // ========================================
                    // F64 Instructions (64-bit floating point)
                    // ========================================
                    Instruction::F64Const(bits) => {
                        #[cfg(feature = "tracing")]
                        trace!("F64Const: pushing {}", f64::from_bits(bits));
                        operand_stack.push(Value::F64(FloatBits64(bits)));
                    }
                    Instruction::F32Load(mem_arg) => {
                        #[cfg(feature = "tracing")]
                        trace!("F32Load: stack before pop has {} elements", operand_stack.len());
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 4)? as u32;
                            #[cfg(feature = "tracing")]
                            trace!("F32Load: addr={}, offset={}, mem_idx={}", addr, offset, mem_arg.memory_index);
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let bits = u32::from_le_bytes(buffer);
                                            #[cfg(feature = "tracing")]
                                            trace!("F32Load: read bytes {:?}, bits={:#x}, pushing F32", buffer, bits);
                                            operand_stack.push(Value::F32(FloatBits32(bits)));
                                            #[cfg(feature = "tracing")]
                                            trace!("F32Load: stack after push has {} elements", operand_stack.len());
                                        }
                                        Err(e) => {
                                            #[cfg(feature = "tracing")]
                                            error!("F32Load: memory read error: {:?}", e);
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(e) => {
                                    #[cfg(feature = "tracing")]
                                    error!("F32Load: memory access error: {:?}", e);
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        } else {
                            #[cfg(feature = "tracing")]
                            error!("F32Load: stack was empty or top was not I32!");
                        }
                    }
                    Instruction::F32Store(mem_arg) => {
                        if let (Some(Value::F32(bits)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 4)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = bits.0.to_le_bytes();
                                    if memory.write_shared(offset, &bytes).is_err() {
                                        return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                    }
                                }
                                Err(_e) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::F64Load(mem_arg) => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 8)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(offset, &mut buffer) {
                                        Ok(()) => {
                                            let bits = u64::from_le_bytes(buffer);
                                            operand_stack.push(Value::F64(FloatBits64(bits)));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    Instruction::F64Store(mem_arg) => {
                        if let (Some(Value::F64(bits)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let offset = calculate_effective_address(addr, mem_arg.offset, 8)? as u32;
                            match instance.memory(mem_arg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = bits.0.to_le_bytes();
                                    if memory.write_shared(offset, &bytes).is_err() {
                                        return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                    }
                                }
                                Err(_e) => {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                            }
                        }
                    }
                    // F32 Arithmetic operations
                    Instruction::F32Add => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.value() + b.value();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Sub => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.value() - b.value();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Mul => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.value() * b.value();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Div => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.value() / b.value();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    // F32 Unary operations
                    Instruction::F32Abs => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            let result = a.value().abs();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Neg => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            let result = -a.value();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Ceil => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            let result = a.value().ceil();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Floor => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            let result = a.value().floor();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Trunc => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            let result = a.value().trunc();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Nearest => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            let f = a.value();
                            // Round to nearest even (banker's rounding)
                            let result = if f.fract().abs() == 0.5 {
                                let floor = f.floor();
                                if floor as i32 % 2 == 0 { floor } else { f.ceil() }
                            } else {
                                f.round()
                            };
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Sqrt => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            let result = a.value().sqrt();
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Min => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let fa = a.value();
                            let fb = b.value();
                            // WebAssembly spec: If either operand is NaN, return NaN
                            // If both are zero with different signs, return -0.0
                            let result = if fa.is_nan() || fb.is_nan() {
                                f32::NAN
                            } else if fa == 0.0 && fb == 0.0 {
                                if fa.is_sign_negative() || fb.is_sign_negative() { -0.0 } else { 0.0 }
                            } else {
                                fa.min(fb)
                            };
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Max => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let fa = a.value();
                            let fb = b.value();
                            // WebAssembly spec: If either operand is NaN, return NaN
                            // If both are zero with different signs, return +0.0
                            let result = if fa.is_nan() || fb.is_nan() {
                                f32::NAN
                            } else if fa == 0.0 && fb == 0.0 {
                                if fa.is_sign_positive() || fb.is_sign_positive() { 0.0 } else { -0.0 }
                            } else {
                                fa.max(fb)
                            };
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32Copysign => {
                        if let (Some(Value::F32(b)), Some(Value::F32(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = a.value().copysign(b.value());
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    // F64 Arithmetic operations
                    Instruction::F64Add => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = f64::from_bits(a.0) + f64::from_bits(b.0);
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Sub => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = f64::from_bits(a.0) - f64::from_bits(b.0);
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Mul => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = f64::from_bits(a.0) * f64::from_bits(b.0);
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Div => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = f64::from_bits(a.0) / f64::from_bits(b.0);
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    // F64 Comparison operations
                    Instruction::F64Eq => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if f64::from_bits(a.0) == f64::from_bits(b.0) { 1i32 } else { 0i32 };
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F64Ne => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if f64::from_bits(a.0) != f64::from_bits(b.0) { 1i32 } else { 0i32 };
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F64Lt => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if f64::from_bits(a.0) < f64::from_bits(b.0) { 1i32 } else { 0i32 };
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F64Gt => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if f64::from_bits(a.0) > f64::from_bits(b.0) { 1i32 } else { 0i32 };
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F64Le => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if f64::from_bits(a.0) <= f64::from_bits(b.0) { 1i32 } else { 0i32 };
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::F64Ge => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = if f64::from_bits(a.0) >= f64::from_bits(b.0) { 1i32 } else { 0i32 };
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    // F64 Unary operations
                    Instruction::F64Abs => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            let result = f64::from_bits(a.0).abs();
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Neg => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            let result = -f64::from_bits(a.0);
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Ceil => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            let result = f64::from_bits(a.0).ceil();
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Floor => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            let result = f64::from_bits(a.0).floor();
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Trunc => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            let result = f64::from_bits(a.0).trunc();
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Nearest => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            let f = f64::from_bits(a.0);
                            let result = if f.fract().abs() == 0.5 {
                                let floor = f.floor();
                                if floor as i64 % 2 == 0 { floor } else { f.ceil() }
                            } else {
                                f.round()
                            };
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Sqrt => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            let result = f64::from_bits(a.0).sqrt();
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Min => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let fa = f64::from_bits(a.0);
                            let fb = f64::from_bits(b.0);
                            let result = if fa.is_nan() || fb.is_nan() {
                                f64::NAN
                            } else if fa == 0.0 && fb == 0.0 {
                                if fa.is_sign_negative() || fb.is_sign_negative() { -0.0 } else { 0.0 }
                            } else {
                                fa.min(fb)
                            };
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Max => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let fa = f64::from_bits(a.0);
                            let fb = f64::from_bits(b.0);
                            let result = if fa.is_nan() || fb.is_nan() {
                                f64::NAN
                            } else if fa == 0.0 && fb == 0.0 {
                                if fa.is_sign_positive() || fb.is_sign_positive() { 0.0 } else { -0.0 }
                            } else {
                                fa.max(fb)
                            };
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64Copysign => {
                        if let (Some(Value::F64(b)), Some(Value::F64(a))) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = f64::from_bits(a.0).copysign(f64::from_bits(b.0));
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    // F32 Conversion operations
                    Instruction::F32ConvertI32S => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = a as f32;
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32ConvertI32U => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = (a as u32) as f32;
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32ConvertI64S => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = a as f32;
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F32ConvertI64U => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = (a as u64) as f32;
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    // F64 Conversion operations
                    Instruction::F64ConvertI32S => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = a as f64;
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64ConvertI32U => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            let result = (a as u32) as f64;
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64ConvertI64S => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = a as f64;
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64ConvertI64U => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            let result = (a as u64) as f64;
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F64PromoteF32 => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            let result = f32::from_bits(a.0) as f64;
                            operand_stack.push(Value::F64(FloatBits64(result.to_bits())));
                        }
                    }
                    Instruction::F32DemoteF64 => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            let result = f64::from_bits(a.0) as f32;
                            operand_stack.push(Value::F32(FloatBits32(result.to_bits())));
                        }
                    }
                    Instruction::F64ReinterpretI64 => {
                        if let Some(Value::I64(a)) = operand_stack.pop() {
                            operand_stack.push(Value::F64(FloatBits64(a as u64)));
                        }
                    }
                    Instruction::I64ReinterpretF64 => {
                        if let Some(Value::F64(a)) = operand_stack.pop() {
                            operand_stack.push(Value::I64(a.0 as i64));
                        }
                    }
                    Instruction::I32ReinterpretF32 => {
                        if let Some(Value::F32(a)) = operand_stack.pop() {
                            operand_stack.push(Value::I32(a.0 as i32));
                        }
                    }
                    Instruction::F32ReinterpretI32 => {
                        if let Some(Value::I32(a)) = operand_stack.pop() {
                            operand_stack.push(Value::F32(FloatBits32(a as u32)));
                        }
                    }
                    Instruction::If { block_type_idx } => {
                        block_depth += 1;
                        // Record stack height as what it will be AFTER condition is consumed
                        // The condition is still on stack, so entry height = len() - 1
                        block_stack.push(("if", pc, block_type_idx, operand_stack.len().saturating_sub(1)));
                        #[cfg(feature = "tracing")]
                        trace!("If: block_type_idx={}, depth now {}, stack_height={}",
                               block_type_idx, block_depth, operand_stack.len().saturating_sub(1));
                        // Pop condition
                        let condition_val = operand_stack.pop();
                        if let Some(Value::I32(condition)) = condition_val {
                            #[cfg(feature = "tracing")]

                            trace!("If: condition = {}", condition != 0);
                            if condition == 0 {
                                // Condition is false, skip to else or end
                                let mut depth = 1;
                                let mut new_pc = pc + 1;
                                #[cfg(feature = "tracing")]

                                trace!("If: skipping to else/end, starting from pc={}", new_pc);

                                while new_pc < instructions.len() && depth > 0 {
                                    if let Some(instr) = instructions.get(new_pc) {
                                        match instr {
                                            wrt_foundation::types::Instruction::If { .. } |
                                            wrt_foundation::types::Instruction::Block { .. } |
                                            wrt_foundation::types::Instruction::Loop { .. } |
                                            wrt_foundation::types::Instruction::Try { .. } |
                                            wrt_foundation::types::Instruction::TryTable { .. } => {
                                                depth += 1;
                                            }
                                            wrt_foundation::types::Instruction::End => {
                                                depth -= 1;
                                                if depth == 0 {
                                                    // Found matching end - jump just before it so we execute the End
                                                    #[cfg(feature = "tracing")]

                                                    trace!("If: found matching end at pc={}", new_pc);
                                                    pc = new_pc - 1; // -1 because we'll +1 at end of loop
                                                    break;
                                                }
                                            }
                                            wrt_foundation::types::Instruction::Else => {
                                                if depth == 1 {
                                                    // Found else at same level - execute else block
                                                    #[cfg(feature = "tracing")]

                                                    trace!("If: found else at pc={}, will execute else block", new_pc);
                                                    pc = new_pc; // Jump to else, will +1 to start after else
                                                    break;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    new_pc += 1;
                                }
                            } else {
                                #[cfg(feature = "tracing")]

                                trace!("If: executing then block");
                            }
                        }
                    }
                    Instruction::Else => {
                        #[cfg(feature = "tracing")]

                        trace!("Else: skipping to end of if block");
                        // When we hit Else during execution, it means we executed the then block
                        // and need to skip over the else block to the end
                        let mut depth = 1;
                        let mut new_pc = pc + 1;

                        while new_pc < instructions.len() && depth > 0 {
                            if let Some(instr) = instructions.get(new_pc) {
                                match instr {
                                    wrt_foundation::types::Instruction::If { .. } |
                                    wrt_foundation::types::Instruction::Block { .. } |
                                    wrt_foundation::types::Instruction::Loop { .. } |
                                    wrt_foundation::types::Instruction::Try { .. } |
                                    wrt_foundation::types::Instruction::TryTable { .. } => {
                                        depth += 1;
                                    }
                                    wrt_foundation::types::Instruction::End => {
                                        depth -= 1;
                                        if depth == 0 {
                                            // Found matching end - jump just before it
                                            #[cfg(feature = "tracing")]

                                            trace!("Else: found matching end at pc={}", new_pc);
                                            pc = new_pc - 1; // -1 because we'll +1 at end of loop
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            new_pc += 1;
                        }
                    }
                    Instruction::Block { block_type_idx } => {
                        block_depth += 1;
                        block_stack.push(("block", pc, block_type_idx, operand_stack.len()));
                        #[cfg(feature = "tracing")]
                        trace!("Block: block_type_idx={}, depth now {}, stack_height={}", block_type_idx, block_depth, operand_stack.len());
                        // Trace block entry in func 76
                        #[cfg(feature = "tracing")]
                        if func_idx == 76 {
                            trace!(pc = pc, block_depth = block_depth, "[BLOCK] func_idx=76 block entry");
                        }
                        // Trace block entry in func 222
                        #[cfg(feature = "tracing")]
                        if func_idx == 222 {
                            trace!(pc = pc, block_depth = block_depth, "[BLOCK-222] Block entry");
                        }
                        // Just execute through the block - End will decrement depth
                    }
                    Instruction::Loop { block_type_idx } => {
                        block_depth += 1;
                        block_stack.push(("loop", pc, block_type_idx, operand_stack.len()));
                        #[cfg(feature = "tracing")]
                        trace!("Loop: block_type_idx={}, depth now {}, start_pc={}, stack_height={}", block_type_idx, block_depth, pc, operand_stack.len());
                        // Trace loop entry in func 76
                        #[cfg(feature = "tracing")]
                        if func_idx == 76 {
                            trace!(pc = pc, block_depth = block_depth, "[LOOP] func_idx=76 loop entry");
                        }
                        // Trace loop entry in func 222
                        #[cfg(feature = "tracing")]
                        if func_idx == 222 {
                            trace!(pc = pc, block_depth = block_depth, "[LOOP-222] Loop entry");
                        }
                        // Just execute through - Br will handle jumping back to start
                    }
                    Instruction::Br(label_idx) => {
                        #[cfg(feature = "tracing")]
                        trace!("Br: label_idx={} (unconditional branch)", label_idx);
                        // Trace Br in func 222
                        #[cfg(feature = "tracing")]
                        if func_idx == 222 {
                            trace!(
                                pc = pc,
                                label_idx = label_idx,
                                block_depth = block_depth,
                                block_stack_len = block_stack.len(),
                                "[BR-222] Branch instruction"
                            );
                            for (i, (btype, bpc, _, _)) in block_stack.iter().enumerate() {
                                trace!(index = i, block_type = btype, block_pc = bpc, "[BR-222] Block stack entry");
                            }
                        }
                        // Trace Br in func 76 (extend_desugared)
                        #[cfg(feature = "tracing")]
                        if func_idx == 76 {
                            trace!(
                                pc = pc,
                                label_idx = label_idx,
                                block_stack_len = block_stack.len(),
                                "[BR] func_idx=76 branch"
                            );
                            for (i, (btype, bpc, _, height)) in block_stack.iter().enumerate() {
                                trace!(index = i, block_type = btype, block_pc = bpc, height = height, "[BR] Block stack entry");
                            }
                        }

                        // Get the target block from the block_stack
                        // label_idx=0 means innermost block, 1 means next outer, etc.
                        if (label_idx as usize) < block_stack.len() {
                            let stack_idx = block_stack.len() - 1 - (label_idx as usize);
                            let (block_type, start_pc, block_type_idx, entry_stack_height) = block_stack[stack_idx];

                            // Determine how many values to preserve based on block type
                            // For loops: branch to beginning preserves values matching PARAMETER count
                            // For blocks: branch to end preserves the block's RESULT values
                            let values_to_preserve = if block_type == "loop" {
                                // Loop branches preserve parameter count (input arity)
                                match block_type_idx {
                                    0x40 => 0, // empty type - no params
                                    0x7F | 0x7E | 0x7D | 0x7C | 0x7B => 0, // inline value types: 0 params
                                    0x70 | 0x6F => 0, // funcref, externref: 0 params
                                    _ => {
                                        // Type index - look up actual param count from module types
                                        if let Some(func_type) = module.types.get(block_type_idx as usize) {
                                            func_type.params.len()
                                        } else {
                                            0
                                        }
                                    }
                                }
                            } else {
                                match block_type_idx {
                                    0x40 => 0, // empty type - no return
                                    0x7F | 0x7E | 0x7D | 0x7C | 0x7B => 1, // i32, i64, f32, f64, v128
                                    0x70 | 0x6F => 1, // funcref, externref
                                    _ => {
                                        // Type index - look up actual result count from module types
                                        if let Some(func_type) = module.types.get(block_type_idx as usize) {
                                            func_type.results.len()
                                        } else {
                                            1 // Fallback if type not found
                                        }
                                    }
                                }
                            };

                            // Save the values to preserve from top of stack
                            let mut preserved_values = Vec::new();
                            #[cfg(feature = "tracing")]
                            trace!(
                                pc = pc,
                                block_type_idx = format_args!("0x{:02X}", block_type_idx),
                                values_to_preserve = values_to_preserve,
                                stack_len = operand_stack.len(),
                                entry_height = entry_stack_height,
                                "[BR-VALUE] Branch value handling"
                            );
                            for _ in 0..values_to_preserve {
                                if let Some(v) = operand_stack.pop() {
                                    #[cfg(feature = "tracing")]
                                    trace!(value = ?v, "[BR-VALUE] Saving value");
                                    preserved_values.push(v);
                                }
                            }

                            // Clear stack down to the entry height
                            while operand_stack.len() > entry_stack_height {
                                let _ = operand_stack.pop();
                            }
                            #[cfg(feature = "tracing")]
                            trace!(stack_len = operand_stack.len(), "[BR-VALUE] After clearing");

                            if block_type == "loop" {
                                // For Loop: jump backward to the loop start
                                // IMPORTANT: Pop all inner blocks from the stack since we're
                                // jumping out of them
                                let blocks_to_pop = label_idx as usize;
                                for _ in 0..blocks_to_pop {
                                    if !block_stack.is_empty() {
                                        block_stack.pop();
                                        block_depth -= 1;
                                    }
                                }
                                #[cfg(feature = "tracing")]
                                trace!("Br: jumping backward to loop start at pc={}, popped {} inner blocks", start_pc, blocks_to_pop);
                                pc = start_pc;  // Will +1 at end of iteration, so we execute the Loop instruction again
                            } else {
                                // For Block/If: jump forward to the End
                                // IMPORTANT: Pop all inner blocks from the stack since we're
                                // jumping over their End instructions
                                let blocks_to_pop = label_idx as usize;
                                for _ in 0..blocks_to_pop {
                                    if !block_stack.is_empty() {
                                        let popped = block_stack.pop();
                                        block_depth -= 1;
                                        #[cfg(feature = "tracing")]
                                        if func_idx == 76 {
                                            trace!(popped = ?popped, block_depth = block_depth, "[BR-FWD] Popping inner block");
                                        }
                                    }
                                }

                                // Scan forward to find the target block's End
                                // We need to skip past (label_idx + 1) End instructions at depth 0
                                let mut target_depth = label_idx as i32 + 1;
                                let mut new_pc = pc + 1;
                                let mut depth = 0;

                                #[cfg(feature = "tracing")]
                                if func_idx == 76 {
                                    trace!(new_pc = new_pc, target_depth = target_depth, "[BR-FWD] Starting scan");
                                }

                                while new_pc < instructions.len() && target_depth > 0 {
                                    if let Some(instr) = instructions.get(new_pc) {
                                        match instr {
                                            wrt_foundation::types::Instruction::Block { .. } |
                                            wrt_foundation::types::Instruction::Loop { .. } |
                                            wrt_foundation::types::Instruction::If { .. } |
                                            wrt_foundation::types::Instruction::Try { .. } |
                                            wrt_foundation::types::Instruction::TryTable { .. } => {
                                                depth += 1;
                                                #[cfg(feature = "tracing")]
                                                if func_idx == 76 {
                                                    trace!(pc = new_pc, depth = depth, "[BR-FWD] Block/Loop/If/Try");
                                                }
                                            }
                                            wrt_foundation::types::Instruction::End => {
                                                #[cfg(feature = "tracing")]
                                                if func_idx == 76 {
                                                    trace!(pc = new_pc, depth = depth, target_depth = target_depth, "[BR-FWD] End");
                                                }
                                                if depth == 0 {
                                                    target_depth -= 1;
                                                    if target_depth == 0 {
                                                        #[cfg(feature = "tracing")]
                                                        trace!("Br: jumping forward to pc={} (end of {} block)", new_pc, block_type);
                                                        #[cfg(feature = "tracing")]
                                                        if func_idx == 76 {
                                                            trace!(pc = new_pc, "[BR-FWD] Jumping to");
                                                        }
                                                        pc = new_pc - 1;
                                                        break;
                                                    }
                                                } else {
                                                    depth -= 1;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    new_pc += 1;
                                }
                            }

                            // Restore preserved values back to stack (in reverse order)
                            #[cfg(feature = "tracing")]
                            trace!(count = preserved_values.len(), "[BR-VALUE] Restoring values");
                            for v in preserved_values.into_iter().rev() {
                                #[cfg(feature = "tracing")]
                                trace!(value = ?v, "[BR-VALUE] Restoring value");
                                operand_stack.push(v);
                            }
                            #[cfg(feature = "tracing")]
                            trace!(stack_len = operand_stack.len(), "[BR-VALUE] After restore");
                        } else {
                            // Branch targets the function level (return from function)
                            // This happens when label_idx >= block_stack.len()
                            #[cfg(feature = "tracing")]
                            trace!("Br: label_idx {} targeting function level (return)", label_idx);

                            // Determine how many values to preserve based on function's return type
                            let values_to_preserve = if let Some(func_type) = module.types.get(func.type_idx as usize) {
                                func_type.results.len()
                            } else {
                                0
                            };

                            // Save the values to preserve from top of stack
                            let mut preserved_values = Vec::new();
                            for _ in 0..values_to_preserve {
                                if let Some(v) = operand_stack.pop() {
                                    preserved_values.push(v);
                                }
                            }

                            // Clear the rest of the stack
                            operand_stack.clear();

                            // Restore preserved values (in reverse order)
                            for v in preserved_values.into_iter().rev() {
                                operand_stack.push(v);
                            }

                            // Return from function
                            break;
                        }
                    }
                    Instruction::BrIf(label_idx) => {
                        if let Some(Value::I32(condition)) = operand_stack.pop() {
                            #[cfg(feature = "tracing")]
                            trace!("BrIf: label_idx={}, condition={}", label_idx, condition != 0);
                            // Trace BrIf in specific functions for debugging
                            #[cfg(feature = "tracing")]
                            if func_idx == 76 || func_idx == 211 || func_idx == 222 {
                                trace!(
                                    func_idx = func_idx,
                                    pc = pc,
                                    label_idx = label_idx,
                                    condition = condition,
                                    will_branch = (condition != 0),
                                    "[BRIF] Conditional branch check"
                                );
                            }
                            if condition != 0 {
                                // Branch conditionally - same logic as Br
                                if (label_idx as usize) < block_stack.len() {
                                    let stack_idx = block_stack.len() - 1 - (label_idx as usize);
                                    let (block_type, start_pc, block_type_idx, entry_stack_height) = block_stack[stack_idx];

                                    // Determine how many values to preserve based on block type
                                    // For loops: preserve PARAMETER count; for blocks: preserve RESULT count
                                    let values_to_preserve = if block_type == "loop" {
                                        // Loop branches preserve parameter count (input arity)
                                        match block_type_idx {
                                            0x40 => 0, // empty type - no params
                                            0x7F | 0x7E | 0x7D | 0x7C | 0x7B => 0, // inline value types: 0 params
                                            0x70 | 0x6F => 0, // funcref, externref: 0 params
                                            _ => {
                                                // Type index - look up actual param count from module types
                                                if let Some(func_type) = module.types.get(block_type_idx as usize) {
                                                    func_type.params.len()
                                                } else {
                                                    0
                                                }
                                            }
                                        }
                                    } else {
                                        match block_type_idx {
                                            0x40 => 0, // empty type - no return
                                            0x7F | 0x7E | 0x7D | 0x7C | 0x7B => 1, // i32, i64, f32, f64, v128
                                            0x70 | 0x6F => 1, // funcref, externref
                                            _ => {
                                                // Type index - look up actual result count
                                                if let Some(func_type) = module.types.get(block_type_idx as usize) {
                                                    func_type.results.len()
                                                } else {
                                                    1
                                                }
                                            }
                                        }
                                    };

                                    // Save the values to preserve from top of stack
                                    let mut preserved_values = Vec::new();
                                    for _ in 0..values_to_preserve {
                                        if let Some(v) = operand_stack.pop() {
                                            preserved_values.push(v);
                                        }
                                    }

                                    // Clear stack down to the entry height
                                    while operand_stack.len() > entry_stack_height {
                                        let _ = operand_stack.pop();
                                    }

                                    if block_type == "loop" {
                                        // For Loop: jump backward to the loop start
                                        // IMPORTANT: Pop all inner blocks from the stack since we're
                                        // jumping out of them
                                        let blocks_to_pop = label_idx as usize;
                                        for _ in 0..blocks_to_pop {
                                            if !block_stack.is_empty() {
                                                block_stack.pop();
                                                block_depth -= 1;
                                            }
                                        }
                                        #[cfg(feature = "tracing")]
                                        trace!("BrIf: jumping backward to loop start at pc={}, popped {} inner blocks", start_pc, blocks_to_pop);
                                        pc = start_pc;
                                    } else {
                                        // For Block/If: jump forward to the End
                                        // Pop inner blocks from stack first
                                        let blocks_to_pop = label_idx as usize;
                                        for _ in 0..blocks_to_pop {
                                            if !block_stack.is_empty() {
                                                block_stack.pop();
                                                block_depth -= 1;
                                            }
                                        }

                                        let mut target_depth = label_idx as i32 + 1;
                                        let mut new_pc = pc + 1;
                                        let mut depth = 0;

                                        while new_pc < instructions.len() && target_depth > 0 {
                                            if let Some(instr) = instructions.get(new_pc) {
                                                match instr {
                                                    wrt_foundation::types::Instruction::Block { .. } |
                                                    wrt_foundation::types::Instruction::Loop { .. } |
                                                    wrt_foundation::types::Instruction::If { .. } |
                                                    wrt_foundation::types::Instruction::Try { .. } |
                                                    wrt_foundation::types::Instruction::TryTable { .. } => {
                                                        depth += 1;
                                                    }
                                                    wrt_foundation::types::Instruction::End => {
                                                        if depth == 0 {
                                                            target_depth -= 1;
                                                            if target_depth == 0 {
                                                                #[cfg(feature = "tracing")]
                                                                trace!("BrIf: jumping forward to pc={} (end of {} block)", new_pc, block_type);
                                                                pc = new_pc - 1;
                                                                break;
                                                            }
                                                        } else {
                                                            depth -= 1;
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            new_pc += 1;
                                        }
                                    }

                                    // Restore preserved values back to stack (in reverse order)
                                    for v in preserved_values.into_iter().rev() {
                                        operand_stack.push(v);
                                    }
                                } else {
                                    // Branch targets the function level (return from function)
                                    // This happens when label_idx >= block_stack.len()
                                    #[cfg(feature = "tracing")]
                                    trace!("BrIf: label_idx {} targeting function level (return)", label_idx);

                                    // Determine how many values to preserve based on function's return type
                                    let values_to_preserve = if let Some(func_type) = module.types.get(func.type_idx as usize) {
                                        func_type.results.len()
                                    } else {
                                        0
                                    };

                                    // Save the values to preserve from top of stack
                                    let mut preserved_values = Vec::new();
                                    for _ in 0..values_to_preserve {
                                        if let Some(v) = operand_stack.pop() {
                                            preserved_values.push(v);
                                        }
                                    }

                                    // Clear the rest of the stack
                                    operand_stack.clear();

                                    // Restore preserved values (in reverse order)
                                    for v in preserved_values.into_iter().rev() {
                                        operand_stack.push(v);
                                    }

                                    // Return from function
                                    break;
                                }
                            } else {
                                #[cfg(feature = "tracing")]

                                trace!("BrIf: condition false, not branching");
                            }
                        }
                    }
                    Instruction::MemorySize(memory_idx) => {
                        // Get the memory size in pages (1 page = 64KB = 65536 bytes)
                        // Use INSTANCE memory (initialized with data segments), not module memory
                        match instance.memory(memory_idx as u32) {
                            Ok(memory_wrapper) => {
                                let memory = &memory_wrapper.0;
                                let size_in_pages = memory.size();
                                #[cfg(feature = "tracing")]
                                trace!(
                                    memory_idx = memory_idx,
                                    size_in_pages = size_in_pages,
                                    "[MemorySize] Retrieved memory size"
                                );
                                operand_stack.push(Value::I32(size_in_pages as i32));
                            }
                            Err(e) => {
                                #[cfg(feature = "tracing")]
                                trace!("MemorySize: memory[{}] not found: {:?}, pushing 0", memory_idx, e);
                                operand_stack.push(Value::I32(0));
                            }
                        }
                    }
                    Instruction::MemoryGrow(memory_idx) => {
                        // Pop the number of pages to grow
                        if let Some(Value::I32(delta)) = operand_stack.pop() {
                            if delta < 0 {
                                // Negative delta is invalid, return -1 (failure)
                                #[cfg(feature = "tracing")]
                                trace!("MemoryGrow: negative delta {}, pushing -1", delta);
                                operand_stack.push(Value::I32(-1));
                            } else {
                                // Use instance memory for grow (has initialized data segments)
                                match instance.memory(memory_idx as u32) {
                                    Ok(memory_wrapper) => {
                                        let memory = &memory_wrapper.0;
                                        let current_size = memory.size();
                                        #[cfg(feature = "tracing")]
                                        trace!(
                                            memory_idx = memory_idx,
                                            delta = delta,
                                            current_size = current_size,
                                            "[MemoryGrow] Attempting to grow memory"
                                        );
                                        match memory.grow_shared(delta as u32) {
                                            Ok(prev_pages) => {
                                                #[cfg(feature = "tracing")]
                                                trace!(
                                                    memory_idx = memory_idx,
                                                    prev_pages = prev_pages,
                                                    new_pages = prev_pages + delta as u32,
                                                    "[MemoryGrow] Success"
                                                );
                                                operand_stack.push(Value::I32(prev_pages as i32));
                                            }
                                            Err(e) => {
                                                #[cfg(feature = "tracing")]
                                                warn!(
                                                    memory_idx = memory_idx,
                                                    error = ?e,
                                                    "[MemoryGrow] Failed"
                                                );
                                                operand_stack.push(Value::I32(-1));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        #[cfg(feature = "tracing")]
                                        trace!("MemoryGrow: memory[{}] not found: {:?}", memory_idx, e);
                                        operand_stack.push(Value::I32(-1));
                                    }
                                }
                            }
                        }
                    }
                    Instruction::MemoryCopy(dst_mem_idx, src_mem_idx) => {
                        // Pop size, src, dest from stack (in that order per wasm spec)
                        if let (Some(Value::I32(size)), Some(Value::I32(src)), Some(Value::I32(dest))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            #[cfg(feature = "tracing")]
                            trace!(
                                dest = format_args!("{:#x}", dest),
                                src = format_args!("{:#x}", src),
                                size = size,
                                dst_mem_idx = dst_mem_idx,
                                src_mem_idx = src_mem_idx,
                                "[MemoryCopy] Starting copy operation"
                            );

                            // For now, only support same-memory copy (most common case)
                            // Multi-memory support can be added later
                            if dst_mem_idx != src_mem_idx {
                                #[cfg(feature = "tracing")]
                                trace!("MemoryCopy: cross-memory copy not yet implemented");
                                return Err(wrt_error::Error::runtime_error("Cross-memory copy not yet implemented"));
                            }

                            // Per WebAssembly spec: bounds check MUST happen before checking size==0
                            // If size == 0 AND (dest > memory.size OR src > memory.size): TRAP
                            // If size > 0 AND ((dest + size) > memory.size OR (src + size) > memory.size): TRAP
                            #[cfg(any(feature = "std", feature = "alloc"))]
                            {
                                let memory_wrapper = instance.memory(dst_mem_idx)?;
                                let memory = &memory_wrapper.0;
                                let memory_size = memory.size_in_bytes() as u32;
                                let dest_u32 = dest as u32;
                                let src_u32 = src as u32;
                                let size_u32 = size as u32;

                                if size_u32 == 0 {
                                    // For size 0, check if offsets are within bounds (can be equal to size)
                                    if dest_u32 > memory_size || src_u32 > memory_size {
                                        return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                    }
                                    // No-op for zero size copy after bounds check passes
                                    continue;
                                }

                                // For size > 0, check if (offset + size) overflows or exceeds memory size
                                let dest_end = dest_u32.checked_add(size_u32)
                                    .ok_or_else(|| wrt_error::Error::runtime_trap("out of bounds memory access"))?;
                                let src_end = src_u32.checked_add(size_u32)
                                    .ok_or_else(|| wrt_error::Error::runtime_trap("out of bounds memory access"))?;

                                if dest_end > memory_size || src_end > memory_size {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }

                                let size_usize = size_u32 as usize;

                                // Read source data into temp buffer (handles overlapping regions)
                                let mut buffer = vec![0u8; size_usize];
                                if let Err(e) = memory.read(src_u32, &mut buffer) {
                                    #[cfg(feature = "tracing")]
                                    trace!("MemoryCopy: read failed: {:?}", e);
                                    return Err(e);
                                }

                                // Write to destination using write_shared (thread-safe)
                                if let Err(e) = memory.write_shared(dest_u32, &buffer) {
                                    #[cfg(feature = "tracing")]
                                    trace!("MemoryCopy: write failed: {:?}", e);
                                    return Err(e);
                                }

                                #[cfg(feature = "tracing")]
                                {
                                    trace!(
                                        size = size,
                                        src = format_args!("{:#x}", src),
                                        dest = format_args!("{:#x}", dest),
                                        "[MemoryCopy] SUCCESS"
                                    );
                                    // Show copied data as string if ASCII
                                    if size <= 50 && buffer.iter().all(|&b| b >= 0x20 && b <= 0x7e || b == 0) {
                                        if let Ok(s) = core::str::from_utf8(&buffer) {
                                            trace!(data = %s, "[MemoryCopy] Copied ASCII data");
                                        }
                                    }
                                }
                            }
                            #[cfg(not(any(feature = "std", feature = "alloc")))]
                            return Err(wrt_error::Error::runtime_error("MemoryCopy requires std or alloc feature"));
                        } else {
                            #[cfg(feature = "tracing")]
                            trace!("MemoryCopy: insufficient values on stack");
                        }
                    }
                    Instruction::MemoryFill(mem_idx) => {
                        // Pop size, value, dest from stack (in that order per wasm spec)
                        if let (Some(Value::I32(size)), Some(Value::I32(value)), Some(Value::I32(dest))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            #[cfg(feature = "tracing")]
                            trace!(
                                dest = format_args!("{:#x}", dest),
                                value = format_args!("{:#x}", value),
                                size = size,
                                mem_idx = mem_idx,
                                "[MemoryFill] Starting fill operation"
                            );

                            // Per WebAssembly spec: bounds check MUST happen before checking size==0
                            // If size == 0 AND dest > memory.size: TRAP
                            // If size > 0 AND (dest + size) > memory.size: TRAP
                            let memory_wrapper = instance.memory(mem_idx)?;
                            let memory = &memory_wrapper.0;
                            let memory_size = memory.size_in_bytes() as u32;
                            let dest_u32 = dest as u32;
                            let size_u32 = size as u32;

                            if size_u32 == 0 {
                                // For size 0, check if offset is within bounds (can be equal to size)
                                if dest_u32 > memory_size {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                                // No-op for zero size fill after bounds check passes
                                continue;
                            }

                            // For size > 0, check if (offset + size) overflows or exceeds memory size
                            let dest_end = dest_u32.checked_add(size_u32)
                                .ok_or_else(|| wrt_error::Error::runtime_trap("out of bounds memory access"))?;

                            if dest_end > memory_size {
                                return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                            }

                            let size_usize = size_u32 as usize;
                            let fill_byte = (value & 0xFF) as u8;

                            // Create buffer filled with the value
                            let buffer = vec![fill_byte; size_usize];

                            // Write to destination using write_shared (thread-safe)
                            if let Err(e) = memory.write_shared(dest_u32, &buffer) {
                                #[cfg(feature = "tracing")]
                                trace!("MemoryFill: write failed: {:?}", e);
                                return Err(e);
                            }

                            #[cfg(feature = "tracing")]
                            trace!(
                                size = size,
                                dest = format_args!("{:#x}", dest),
                                fill_byte = format_args!("{:#x}", fill_byte),
                                "[MemoryFill] SUCCESS"
                            );
                        } else {
                            #[cfg(feature = "tracing")]
                            trace!("MemoryFill: insufficient values on stack");
                        }
                    }
                    Instruction::MemoryInit(data_idx, mem_idx) => {
                        // Pop n (length), s (source offset in data), d (dest offset in memory)
                        if let (Some(Value::I32(n)), Some(Value::I32(s)), Some(Value::I32(d))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            #[cfg(feature = "tracing")]
                            trace!(
                                dest = format_args!("{:#x}", d),
                                src = format_args!("{:#x}", s),
                                len = n,
                                data_idx = data_idx,
                                mem_idx = mem_idx,
                                "[MemoryInit] Starting memory init operation"
                            );

                            // Check if this data segment has been dropped
                            // Per WebAssembly spec, a dropped segment behaves as if it has zero length
                            let is_dropped = self.dropped_data_segments
                                .get(&instance_id)
                                .and_then(|v| v.get(data_idx as usize))
                                .copied()
                                .unwrap_or(false);

                            // Get data segment from module (for length calculation)
                            let data_segment = module.data.get(data_idx as usize)
                                .ok_or_else(|| wrt_error::Error::runtime_trap("out of bounds memory access"))?;

                            // If dropped, treat as zero-length segment
                            let data_len = if is_dropped { 0u32 } else { data_segment.init.len() as u32 };
                            let s_u32 = s as u32;
                            let d_u32 = d as u32;
                            let n_u32 = n as u32;

                            // Per WebAssembly spec: bounds check MUST happen before checking n==0
                            // Get memory for bounds checking
                            let memory_wrapper = instance.memory(mem_idx)?;
                            let memory = &memory_wrapper.0;
                            let memory_size = memory.size_in_bytes() as u32;

                            if n_u32 == 0 {
                                // For n == 0, check if offsets are within bounds (can be equal to size)
                                if s_u32 > data_len || d_u32 > memory_size {
                                    return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                                }
                                // No-op for zero size init after bounds check passes
                                continue;
                            }

                            // For n > 0, check if (offset + n) overflows or exceeds bounds
                            let src_end = s_u32.checked_add(n_u32)
                                .ok_or_else(|| wrt_error::Error::runtime_trap("out of bounds memory access"))?;
                            let dest_end = d_u32.checked_add(n_u32)
                                .ok_or_else(|| wrt_error::Error::runtime_trap("out of bounds memory access"))?;

                            if src_end > data_len {
                                return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                            }
                            if dest_end > memory_size {
                                return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                            }

                            // Copy data from segment to memory
                            #[cfg(any(feature = "std", feature = "alloc"))]
                            {
                                let src_slice = &data_segment.init[s_u32 as usize..src_end as usize];
                                if let Err(e) = memory.write_shared(d_u32, src_slice) {
                                    #[cfg(feature = "tracing")]
                                    trace!("MemoryInit: write failed: {:?}", e);
                                    return Err(e);
                                }
                            }

                            #[cfg(feature = "tracing")]
                            trace!(
                                dest = format_args!("{:#x}", d),
                                src = format_args!("{:#x}", s),
                                len = n,
                                "[MemoryInit] SUCCESS"
                            );
                        } else {
                            #[cfg(feature = "tracing")]
                            trace!("MemoryInit: insufficient values on stack");
                        }
                    }
                    Instruction::DataDrop(data_idx) => {
                        #[cfg(feature = "tracing")]
                        trace!(data_idx = data_idx, "[DataDrop] Dropping data segment");

                        // Validate data segment index
                        if data_idx as usize >= module.data.len() {
                            return Err(wrt_error::Error::runtime_trap("out of bounds memory access"));
                        }

                        // Initialize dropped_data_segments for this instance if not already done
                        let dropped_vec = self.dropped_data_segments
                            .entry(instance_id)
                            .or_insert_with(|| vec![false; module.data.len()]);

                        // Mark the segment as dropped
                        if let Some(dropped) = dropped_vec.get_mut(data_idx as usize) {
                            *dropped = true;
                        }

                        #[cfg(feature = "tracing")]
                        trace!(data_idx = data_idx, "[DataDrop] SUCCESS");
                    }
                    Instruction::BrTable { ref targets, default_target } => {
                        // Pop the index from the stack
                        if let Some(Value::I32(index)) = operand_stack.pop() {
                            #[cfg(feature = "tracing")]

                            trace!("BrTable: index={}, targets.len()={}, default={}",                                      index, targets.len(), default_target);

                            // Select the target based on the index
                            let label_idx = if index >= 0 && (index as usize) < targets.len() {
                                // Use the indexed target
                                match targets.get(index as usize) {
                                    Ok(target) => {
                                        #[cfg(feature = "tracing")]

                                        trace!("BrTable: using target[{}] = {}", index, target);
                                        target
                                    }
                                    Err(_) => {
                                        #[cfg(feature = "tracing")]

                                        trace!("BrTable: error getting target[{}], using default {}", index, default_target);
                                        default_target
                                    }
                                }
                            } else {
                                // Index out of range, use default
                                #[cfg(feature = "tracing")]

                                trace!("BrTable: index {} out of range, using default {}", index, default_target);
                                default_target
                            };

                            // Branch to the selected target (same logic as Br)
                            #[cfg(feature = "tracing")]

                            trace!("BrTable: label_idx={}, block_stack.len()={}", label_idx, block_stack.len());
                            #[cfg(feature = "tracing")]

                            trace!("BrTable: block_stack contents:");
                            for (i, (btype, bpc, _, _)) in block_stack.iter().enumerate() {
                                #[cfg(feature = "tracing")]

                                trace!("  [{}]: {} at pc={}", i, btype, bpc);
                            }
                            if (label_idx as usize) < block_stack.len() {
                                let stack_idx = block_stack.len() - 1 - (label_idx as usize);
                                let (block_type, start_pc, block_type_idx, entry_stack_height) = block_stack[stack_idx];
                                #[cfg(feature = "tracing")]

                                trace!("BrTable: accessing block_stack[{}], target block is {} at pc={}", stack_idx, block_type, start_pc);

                                // Determine how many values to preserve based on block type
                                // For loops: preserve PARAMETER count; for blocks: preserve RESULT count
                                let values_to_preserve = if block_type == "loop" {
                                    // Loop branches preserve parameter count (input arity)
                                    match block_type_idx {
                                        0x40 => 0, // empty type - no params
                                        0x7F | 0x7E | 0x7D | 0x7C | 0x7B => 0, // inline value types: 0 params
                                        0x70 | 0x6F => 0, // funcref, externref: 0 params
                                        _ => {
                                            // Type index - look up actual param count from module types
                                            if let Some(func_type) = module.types.get(block_type_idx as usize) {
                                                func_type.params.len()
                                            } else {
                                                0
                                            }
                                        }
                                    }
                                } else {
                                    match block_type_idx {
                                        0x40 => 0, // empty type - no return
                                        0x7F | 0x7E | 0x7D | 0x7C | 0x7B => 1, // i32, i64, f32, f64, v128
                                        0x70 | 0x6F => 1, // funcref, externref
                                        _ => {
                                            // Type index - look up actual result count
                                            if let Some(func_type) = module.types.get(block_type_idx as usize) {
                                                func_type.results.len()
                                            } else {
                                                1
                                            }
                                        }
                                    }
                                };

                                // Save the values to preserve from top of stack
                                let mut preserved_values = Vec::new();
                                for _ in 0..values_to_preserve {
                                    if let Some(v) = operand_stack.pop() {
                                        preserved_values.push(v);
                                    }
                                }

                                // Clear stack down to the entry height
                                while operand_stack.len() > entry_stack_height {
                                    let _ = operand_stack.pop();
                                }

                                // Pop all inner blocks from the stack since we're jumping over
                                // their End instructions (same as Br instruction logic)
                                let blocks_to_pop = label_idx as usize;
                                for _ in 0..blocks_to_pop {
                                    if !block_stack.is_empty() {
                                        block_stack.pop();
                                        block_depth -= 1;
                                    }
                                }

                                if block_type == "loop" {
                                    // For Loop: jump backward to the loop start
                                    #[cfg(feature = "tracing")]

                                    trace!("BrTable: jumping backward to loop start at pc={}", start_pc);
                                    pc = start_pc;
                                } else {
                                    // For Block/If: jump forward to the End
                                    let mut target_depth = label_idx as i32 + 1;
                                    let mut new_pc = pc + 1;
                                    let mut depth = 0;

                                    while new_pc < instructions.len() && target_depth > 0 {
                                        if let Some(instr) = instructions.get(new_pc) {
                                            match instr {
                                                wrt_foundation::types::Instruction::Block { .. } |
                                                wrt_foundation::types::Instruction::Loop { .. } |
                                                wrt_foundation::types::Instruction::If { .. } |
                                                wrt_foundation::types::Instruction::Try { .. } |
                                                wrt_foundation::types::Instruction::TryTable { .. } => {
                                                    depth += 1;
                                                }
                                                wrt_foundation::types::Instruction::End => {
                                                    if depth == 0 {
                                                        target_depth -= 1;
                                                        if target_depth == 0 {
                                                            #[cfg(feature = "tracing")]

                                                            trace!("BrTable: jumping forward to pc={} (end of {} block)", new_pc, block_type);
                                                            pc = new_pc - 1;
                                                            break;
                                                        }
                                                    } else {
                                                        depth -= 1;
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                        new_pc += 1;
                                    }
                                }

                                // Restore preserved values back to stack (in reverse order)
                                for v in preserved_values.into_iter().rev() {
                                    operand_stack.push(v);
                                }
                            } else {
                                // Branch targets the function level (return from function)
                                // This happens when label_idx >= block_stack.len()
                                #[cfg(feature = "tracing")]
                                trace!("BrTable: label_idx {} targeting function level (return)", label_idx);

                                // Determine how many values to preserve based on function's return type
                                let values_to_preserve = if let Some(func_type) = module.types.get(func.type_idx as usize) {
                                    func_type.results.len()
                                } else {
                                    0
                                };

                                // Save the values to preserve from top of stack
                                let mut preserved_values = Vec::new();
                                for _ in 0..values_to_preserve {
                                    if let Some(v) = operand_stack.pop() {
                                        preserved_values.push(v);
                                    }
                                }

                                // Clear the rest of the stack
                                operand_stack.clear();

                                // Restore preserved values (in reverse order)
                                for v in preserved_values.into_iter().rev() {
                                    operand_stack.push(v);
                                }

                                // Return from function
                                break;
                            }
                        } else {
                            #[cfg(feature = "tracing")]

                            trace!("BrTable: no index on stack");
                        }
                    }
                    Instruction::Return => {
                        #[cfg(feature = "tracing")]
                        trace!("🔙 Return at pc={}", pc);
                        #[cfg(feature = "tracing")]
                        trace!("  Operand stack size: {}", operand_stack.len());
                        #[cfg(feature = "tracing")]
                        trace!("  Instructions executed: {}", instruction_count);
                        // Trace return from specific functions for debugging
                        #[cfg(feature = "tracing")]
                        if func_idx == 76 {
                            trace!(
                                func_idx = func_idx,
                                pc = pc,
                                stack_size = operand_stack.len(),
                                "[RETURN] Function returning"
                            );
                        }
                        break; // Exit function
                    }
                    Instruction::End => {
                        // Decrement block depth first
                        block_depth -= 1;

                        // Check if this is the function's final End instruction
                        // WebAssembly functions have an implicit outer block, so when we hit
                        // an End that brings depth to -1, or we're at the last instruction with depth 0,
                        // we've reached the function end
                        if block_depth < 0 || (pc == instructions.len() - 1 && block_depth == 0) {
                            // This is the function's final End
                            #[cfg(feature = "tracing")]
                            trace!("🔙 End at pc={} (function end)", pc);
                            #[cfg(feature = "tracing")]

                            trace!("  Operand stack size: {}", operand_stack.len());
                            #[cfg(feature = "tracing")]

                            trace!("  Instructions executed: {}", instruction_count);
                            break; // Exit function
                        } else {
                            // This ends a block/loop/if - continue execution
                            if !block_stack.is_empty() {
                                let (block_type, start_pc, _, _) = block_stack.pop().unwrap();
                                #[cfg(feature = "tracing")]
                                trace!("End at pc={} (closes {} from pc={}, depth now {})", pc, block_type, start_pc, block_depth);
                                // Trace End in specific functions for debugging
                                #[cfg(feature = "tracing")]
                                if func_idx == 76 || func_idx == 222 {
                                    trace!(
                                        func_idx = func_idx,
                                        pc = pc,
                                        block_type = %block_type,
                                        start_pc = start_pc,
                                        block_depth = block_depth,
                                        block_stack_len = block_stack.len(),
                                        "[END] Block closing"
                                    );
                                }
                            } else {
                                #[cfg(feature = "tracing")]
                                trace!("End at pc={} (closes block, depth now {})", pc, block_depth);
                            }
                        }
                    }

                    // ========================================
                    // Reference Type Operations
                    // ========================================
                    Instruction::RefNull(value_type) => {
                        // Push a null reference of the specified type
                        use wrt_foundation::types::ValueType;
                        #[cfg(feature = "tracing")]
                        trace!("RefNull: type={:?}", value_type);
                        let null_value = match value_type {
                            // Standard reference types
                            ValueType::FuncRef => Value::FuncRef(None),
                            ValueType::ExternRef => Value::ExternRef(None),
                            // GC abstract heap types (using their Value representations)
                            ValueType::AnyRef => Value::ExternRef(None),   // anyref uses externref repr
                            ValueType::EqRef => Value::I31Ref(None),       // eqref uses i31ref repr
                            ValueType::I31Ref => Value::I31Ref(None),
                            ValueType::StructRef(_) => Value::StructRef(None),
                            ValueType::ArrayRef(_) => Value::ArrayRef(None),
                            ValueType::ExnRef => Value::ExnRef(None),
                            // Non-reference types shouldn't reach here, default to externref
                            _ => Value::ExternRef(None),
                        };
                        operand_stack.push(null_value);
                    }
                    Instruction::RefFunc(func_idx_arg) => {
                        // Push a reference to the function at func_idx
                        #[cfg(feature = "tracing")]
                        trace!("RefFunc: func_idx={}", func_idx_arg);
                        operand_stack.push(Value::FuncRef(Some(wrt_foundation::values::FuncRef { index: func_idx_arg })));
                    }
                    Instruction::RefIsNull => {
                        // Pop reference, push 1 if null, 0 if not null
                        if let Some(ref_val) = operand_stack.pop() {
                            let is_null = match ref_val {
                                Value::FuncRef(None) => 1i32,
                                Value::FuncRef(Some(_)) => 0i32,
                                Value::ExternRef(None) => 1i32,
                                Value::ExternRef(Some(_)) => 0i32,
                                _ => {
                                    #[cfg(feature = "tracing")]
                                    error!("RefIsNull: expected reference type, got {:?}", ref_val);
                                    return Err(wrt_error::Error::runtime_type_mismatch(
                                        "ref.is_null expects a reference type",
                                    ));
                                }
                            };
                            #[cfg(feature = "tracing")]
                            trace!("RefIsNull: result={}", is_null);
                            operand_stack.push(Value::I32(is_null));
                        }
                    }
                    Instruction::RefAsNonNull => {
                        // Pop reference, trap if null, push back if not null
                        if let Some(ref_val) = operand_stack.pop() {
                            match &ref_val {
                                Value::FuncRef(None) | Value::ExternRef(None) => {
                                    #[cfg(feature = "tracing")]
                                    error!("RefAsNonNull: null reference");
                                    return Err(wrt_error::Error::runtime_trap(
                                        "null reference in ref.as_non_null",
                                    ));
                                }
                                Value::FuncRef(Some(_)) | Value::ExternRef(Some(_)) => {
                                    #[cfg(feature = "tracing")]
                                    trace!("RefAsNonNull: non-null reference");
                                    operand_stack.push(ref_val);
                                }
                                _ => {
                                    #[cfg(feature = "tracing")]
                                    error!("RefAsNonNull: expected reference type, got {:?}", ref_val);
                                    return Err(wrt_error::Error::runtime_type_mismatch(
                                        "ref.as_non_null expects a reference type",
                                    ));
                                }
                            }
                        }
                    }
                    Instruction::RefEq => {
                        // Pop two references, push 1 if equal, 0 if not
                        if let (Some(ref2), Some(ref1)) = (operand_stack.pop(), operand_stack.pop()) {
                            let result = match (&ref1, &ref2) {
                                // Two null funcref/externref are equal
                                (Value::FuncRef(None), Value::FuncRef(None)) => 1i32,
                                (Value::ExternRef(None), Value::ExternRef(None)) => 1i32,
                                // Two non-null funcrefs are equal if they reference the same function
                                (Value::FuncRef(Some(f1)), Value::FuncRef(Some(f2))) => {
                                    if f1.index == f2.index { 1i32 } else { 0i32 }
                                }
                                // Two non-null externrefs are equal if they're the same object
                                (Value::ExternRef(Some(e1)), Value::ExternRef(Some(e2))) => {
                                    if e1 == e2 { 1i32 } else { 0i32 }
                                }
                                // Different types or null vs non-null are not equal
                                _ => 0i32,
                            };
                            #[cfg(feature = "tracing")]
                            trace!("RefEq: {:?} == {:?} => {}", ref1, ref2, result);
                            operand_stack.push(Value::I32(result));
                        }
                    }
                    Instruction::BrOnNull(br_label_idx) => {
                        // Pop reference, branch if null, push back if not null
                        if let Some(ref_val) = operand_stack.pop() {
                            let is_null = matches!(&ref_val, Value::FuncRef(None) | Value::ExternRef(None));
                            #[cfg(feature = "tracing")]
                            trace!("BrOnNull: label={}, is_null={}", br_label_idx, is_null);
                            if is_null {
                                // Branch to label - similar to Br instruction
                                if br_label_idx as usize >= block_stack.len() {
                                    // Branch out of function
                                    #[cfg(feature = "tracing")]
                                    trace!("BrOnNull: branching out of function");
                                    break;
                                }
                                // Find the target block in the stack
                                let target_depth = block_stack.len() - 1 - br_label_idx as usize;
                                if let Some((block_type, start_pc, _block_type_idx, entry_stack_height)) = block_stack.get(target_depth).copied() {
                                    // For loops, branch to start; for blocks, branch to end
                                    if block_type == "loop" {
                                        pc = start_pc;
                                    } else {
                                        // Skip to end of block
                                        let mut depth = 1;
                                        let mut search_pc = pc + 1;
                                        while depth > 0 && search_pc < instructions.len() {
                                            #[cfg(feature = "std")]
                                            if let Some(search_instr) = instructions.get(search_pc) {
                                                match search_instr {
                                                    Instruction::Block { .. } | Instruction::Loop { .. } | Instruction::If { .. } | Instruction::Try { .. } | Instruction::TryTable { .. } => depth += 1,
                                                    Instruction::End => depth -= 1,
                                                    _ => {}
                                                }
                                            }
                                            #[cfg(not(feature = "std"))]
                                            if let Ok(search_instr) = instructions.get(search_pc) {
                                                match search_instr {
                                                    Instruction::Block { .. } | Instruction::Loop { .. } | Instruction::If { .. } | Instruction::Try { .. } | Instruction::TryTable { .. } => depth += 1,
                                                    Instruction::End => depth -= 1,
                                                    _ => {}
                                                }
                                            }
                                            if depth > 0 { search_pc += 1; }
                                        }
                                        pc = search_pc;
                                    }
                                    // Restore stack to entry height
                                    while operand_stack.len() > entry_stack_height {
                                        operand_stack.pop();
                                    }
                                }
                                continue;
                            } else {
                                // Not null, push reference back
                                operand_stack.push(ref_val);
                            }
                        }
                    }

                    // ==================== TABLE OPERATIONS ====================
                    Instruction::TableGet(table_idx) => {
                        // table.get: [i32] -> [ref]
                        // Pop index from stack, get element from table at that index
                        if let Some(Value::I32(elem_idx)) = operand_stack.pop() {
                            #[cfg(feature = "tracing")]
                            trace!(
                                table_idx = table_idx,
                                elem_idx = elem_idx,
                                "[TableGet] Getting element from table"
                            );

                            if elem_idx < 0 {
                                return Err(wrt_error::Error::runtime_trap(
                                    "table.get: index cannot be negative",
                                ));
                            }

                            // Get the table from the instance
                            let table = instance.table(table_idx)?;
                            let elem = table.get(elem_idx as u32)?;

                            // Push the element (or null ref) onto the stack
                            let value = match elem {
                                Some(v) => v,
                                None => {
                                    // Return null reference based on table element type
                                    match table.element_type() {
                                        wrt_foundation::types::RefType::Funcref => Value::FuncRef(None),
                                        wrt_foundation::types::RefType::Externref => Value::ExternRef(None),
                                    }
                                }
                            };
                            operand_stack.push(value);

                            #[cfg(feature = "tracing")]
                            trace!(
                                table_idx = table_idx,
                                elem_idx = elem_idx,
                                "[TableGet] SUCCESS"
                            );
                        } else {
                            return Err(wrt_error::Error::runtime_trap(
                                "table.get: expected i32 index on stack",
                            ));
                        }
                    }

                    Instruction::TableSet(table_idx) => {
                        // table.set: [i32 ref] -> []
                        // Pop value, then index from stack; set element in table
                        let value = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.set: expected value on stack")
                        })?;
                        let idx = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.set: expected index on stack")
                        })?;

                        if let Value::I32(elem_idx) = idx {
                            #[cfg(feature = "tracing")]
                            trace!(
                                table_idx = table_idx,
                                elem_idx = elem_idx,
                                value = ?value,
                                "[TableSet] Setting element in table"
                            );

                            if elem_idx < 0 {
                                return Err(wrt_error::Error::runtime_trap(
                                    "table.set: index cannot be negative",
                                ));
                            }

                            // Validate value is a reference type
                            let table_value = match &value {
                                Value::FuncRef(fr) => Some(Value::FuncRef(fr.clone())),
                                Value::ExternRef(er) => Some(Value::ExternRef(er.clone())),
                                _ => {
                                    return Err(wrt_error::Error::runtime_trap(
                                        "table.set: value must be a reference type",
                                    ));
                                }
                            };

                            // Get the table and set the element
                            let table = instance.table(table_idx)?;
                            table.set(elem_idx as u32, table_value)?;

                            #[cfg(feature = "tracing")]
                            trace!(
                                table_idx = table_idx,
                                elem_idx = elem_idx,
                                "[TableSet] SUCCESS"
                            );
                        } else {
                            return Err(wrt_error::Error::runtime_trap(
                                "table.set: expected i32 index",
                            ));
                        }
                    }

                    Instruction::TableSize(table_idx) => {
                        // table.size: [] -> [i32]
                        // Push current table size onto stack
                        #[cfg(feature = "tracing")]
                        trace!(
                            table_idx = table_idx,
                            "[TableSize] Getting table size"
                        );

                        let table = instance.table(table_idx)?;
                        let size = table.size();
                        operand_stack.push(Value::I32(size as i32));

                        #[cfg(feature = "tracing")]
                        trace!(
                            table_idx = table_idx,
                            size = size,
                            "[TableSize] SUCCESS"
                        );
                    }

                    Instruction::TableGrow(table_idx) => {
                        // table.grow: [ref i32] -> [i32]
                        // Pop delta (i32), pop init value (ref), grow table, push old size or -1
                        let delta = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.grow: expected delta on stack")
                        })?;
                        let init_value = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.grow: expected init value on stack")
                        })?;

                        if let Value::I32(delta_val) = delta {
                            #[cfg(feature = "tracing")]
                            trace!(
                                table_idx = table_idx,
                                delta = delta_val,
                                init_value = ?init_value,
                                "[TableGrow] Growing table"
                            );

                            // Negative delta should return -1 (failure)
                            if delta_val < 0 {
                                operand_stack.push(Value::I32(-1));
                            } else {
                                // Validate init value is a reference type
                                match &init_value {
                                    Value::FuncRef(_) | Value::ExternRef(_) => {}
                                    _ => {
                                        return Err(wrt_error::Error::runtime_trap(
                                            "table.grow: init value must be a reference type",
                                        ));
                                    }
                                }

                                let table = instance.table(table_idx)?;
                                match table.grow(delta_val as u32, init_value) {
                                    Ok(old_size) => {
                                        operand_stack.push(Value::I32(old_size as i32));
                                        #[cfg(feature = "tracing")]
                                        trace!(
                                            table_idx = table_idx,
                                            old_size = old_size,
                                            "[TableGrow] SUCCESS"
                                        );
                                    }
                                    Err(_) => {
                                        // Growth failed (e.g., exceeded max size)
                                        operand_stack.push(Value::I32(-1));
                                        #[cfg(feature = "tracing")]
                                        trace!(
                                            table_idx = table_idx,
                                            "[TableGrow] Failed, returning -1"
                                        );
                                    }
                                }
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap(
                                "table.grow: expected i32 delta",
                            ));
                        }
                    }

                    Instruction::TableFill(table_idx) => {
                        // table.fill: [i32 ref i32] -> []
                        // Pop size, value, dest; fill table region with value
                        let size = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.fill: expected size on stack")
                        })?;
                        let value = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.fill: expected value on stack")
                        })?;
                        let dest = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.fill: expected dest on stack")
                        })?;

                        if let (Value::I32(dest_idx), Value::I32(fill_size)) = (&dest, &size) {
                            #[cfg(feature = "tracing")]
                            trace!(
                                table_idx = table_idx,
                                dest = dest_idx,
                                size = fill_size,
                                value = ?value,
                                "[TableFill] Filling table region"
                            );

                            if *dest_idx < 0 || *fill_size < 0 {
                                return Err(wrt_error::Error::runtime_trap(
                                    "table.fill: negative dest or size",
                                ));
                            }

                            // Validate value is a reference type
                            let fill_value = match &value {
                                Value::FuncRef(fr) => Some(Value::FuncRef(fr.clone())),
                                Value::ExternRef(er) => Some(Value::ExternRef(er.clone())),
                                _ => {
                                    return Err(wrt_error::Error::runtime_trap(
                                        "table.fill: value must be a reference type",
                                    ));
                                }
                            };

                            let table = instance.table(table_idx)?;
                            table.fill(*dest_idx as u32, *fill_size as u32, fill_value)?;

                            #[cfg(feature = "tracing")]
                            trace!(
                                table_idx = table_idx,
                                dest = dest_idx,
                                size = fill_size,
                                "[TableFill] SUCCESS"
                            );
                        } else {
                            return Err(wrt_error::Error::runtime_trap(
                                "table.fill: expected i32 values for dest and size",
                            ));
                        }
                    }

                    Instruction::TableCopy(dst_table_idx, src_table_idx) => {
                        // table.copy: [i32 i32 i32] -> []
                        // Pop size, src_offset, dst_offset; copy elements between tables
                        let size = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.copy: expected size on stack")
                        })?;
                        let src_offset = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.copy: expected src offset on stack")
                        })?;
                        let dst_offset = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.copy: expected dst offset on stack")
                        })?;

                        if let (Value::I32(dst_idx), Value::I32(src_idx), Value::I32(copy_size)) =
                            (&dst_offset, &src_offset, &size)
                        {
                            #[cfg(feature = "tracing")]
                            trace!(
                                dst_table = dst_table_idx,
                                src_table = src_table_idx,
                                dst_offset = dst_idx,
                                src_offset = src_idx,
                                size = copy_size,
                                "[TableCopy] Copying table elements"
                            );

                            if *dst_idx < 0 || *src_idx < 0 || *copy_size < 0 {
                                return Err(wrt_error::Error::runtime_trap(
                                    "out of bounds table access",
                                ));
                            }

                            // Handle same-table and cross-table copy
                            if dst_table_idx == src_table_idx {
                                // Same table copy - use the table's copy method
                                let table = instance.table(dst_table_idx)?;
                                table.copy(*dst_idx as u32, *src_idx as u32, *copy_size as u32)?;
                            } else {
                                // Cross-table copy - read from src, write to dst
                                let src_table = instance.table(src_table_idx)?;
                                let dst_table = instance.table(dst_table_idx)?;

                                // Bounds check BEFORE zero-length check (per WebAssembly spec)
                                let src_end = (*src_idx as u32).checked_add(*copy_size as u32)
                                    .ok_or_else(|| wrt_error::Error::runtime_trap(
                                        "out of bounds table access"
                                    ))?;
                                let dst_end = (*dst_idx as u32).checked_add(*copy_size as u32)
                                    .ok_or_else(|| wrt_error::Error::runtime_trap(
                                        "out of bounds table access"
                                    ))?;
                                if src_end > src_table.size() || dst_end > dst_table.size() {
                                    return Err(wrt_error::Error::runtime_trap(
                                        "out of bounds table access"
                                    ));
                                }

                                // Zero-length copy is a no-op (after bounds check)
                                if *copy_size == 0 {
                                    // Continue to next instruction
                                } else {
                                    // Read all source elements first (to handle any overlap scenarios)
                                    let mut temp_elements = Vec::new();
                                    for i in 0..*copy_size as u32 {
                                        let elem = src_table.get(*src_idx as u32 + i)?;
                                        temp_elements.push(elem);
                                    }

                                    // Write to destination table
                                    for (i, elem) in temp_elements.into_iter().enumerate() {
                                        dst_table.set(*dst_idx as u32 + i as u32, elem)?;
                                    }
                                }
                            }

                            #[cfg(feature = "tracing")]
                            trace!(
                                dst_table = dst_table_idx,
                                src_table = src_table_idx,
                                "[TableCopy] SUCCESS"
                            );
                        } else {
                            return Err(wrt_error::Error::runtime_trap(
                                "table.copy: expected i32 values for offsets and size",
                            ));
                        }
                    }

                    Instruction::TableInit(elem_seg_idx, table_idx) => {
                        // table.init: [i32 i32 i32] -> []
                        // Pop size, src_offset (in elem segment), dst_offset (in table)
                        // Initialize table elements from element segment
                        let size = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.init: expected size on stack")
                        })?;
                        let src_offset = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.init: expected src offset on stack")
                        })?;
                        let dst_offset = operand_stack.pop().ok_or_else(|| {
                            wrt_error::Error::runtime_trap("table.init: expected dst offset on stack")
                        })?;

                        if let (Value::I32(dst_idx), Value::I32(src_idx), Value::I32(init_size)) =
                            (&dst_offset, &src_offset, &size)
                        {
                            #[cfg(feature = "tracing")]
                            trace!(
                                elem_seg_idx = elem_seg_idx,
                                table_idx = table_idx,
                                dst_offset = dst_idx,
                                src_offset = src_idx,
                                size = init_size,
                                "[TableInit] Initializing table from element segment"
                            );

                            if *dst_idx < 0 || *src_idx < 0 || *init_size < 0 {
                                return Err(wrt_error::Error::runtime_trap(
                                    "out of bounds table access",
                                ));
                            }

                            // Get the element segment from the module (needed for bounds check)
                            #[cfg(feature = "std")]
                            let elem_segment = module.elements.get(elem_seg_idx as usize)
                                .ok_or_else(|| wrt_error::Error::runtime_trap(
                                    "table.init: invalid element segment index"
                                ))?;
                            #[cfg(not(feature = "std"))]
                            let elem_segment = module.elements.get(elem_seg_idx as usize)
                                .map_err(|_| wrt_error::Error::runtime_trap(
                                    "table.init: invalid element segment index"
                                ))?;

                            // Check if the element segment has been dropped
                            // Dropped segments have effective length 0
                            let effective_elem_len = if instance.is_element_segment_dropped(elem_seg_idx) {
                                0
                            } else {
                                elem_segment.items.len()
                            };

                            // Check bounds in element segment (must happen BEFORE zero-size check per spec)
                            let src_end = (*src_idx as usize).checked_add(*init_size as usize)
                                .ok_or_else(|| wrt_error::Error::runtime_trap(
                                    "out of bounds table access"
                                ))?;
                            if src_end > effective_elem_len {
                                return Err(wrt_error::Error::runtime_trap(
                                    "out of bounds table access",
                                ));
                            }

                            // Get table and check bounds (must happen BEFORE zero-size check per spec)
                            let table = instance.table(table_idx)?;
                            let dst_end = (*dst_idx as u32).checked_add(*init_size as u32)
                                .ok_or_else(|| wrt_error::Error::runtime_trap(
                                    "out of bounds table access"
                                ))?;
                            if dst_end > table.size() {
                                return Err(wrt_error::Error::runtime_trap(
                                    "out of bounds table access",
                                ));
                            }

                            // Handle zero-size init (valid no-op) AFTER bounds checks
                            if *init_size == 0 {
                                #[cfg(feature = "tracing")]
                                trace!("[TableInit] Zero size, no-op");
                                // Continue to next instruction
                            } else {
                                // Copy elements from segment to table
                                for i in 0..*init_size as usize {
                                    let item_idx = *src_idx as usize + i;
                                    let func_idx = elem_segment.items.get(item_idx)
                                        .map_err(|_| wrt_error::Error::runtime_trap(
                                            "table.init: element segment access error"
                                        ))?;

                                    // u32::MAX is sentinel for null reference
                                    let value = if func_idx == u32::MAX {
                                        Some(Value::FuncRef(None))  // null funcref
                                    } else {
                                        Some(Value::FuncRef(Some(
                                            wrt_foundation::values::FuncRef { index: func_idx }
                                        )))
                                    };
                                    table.set(*dst_idx as u32 + i as u32, value)?;
                                }

                                #[cfg(feature = "tracing")]
                                trace!(
                                    elem_seg_idx = elem_seg_idx,
                                    table_idx = table_idx,
                                    "[TableInit] SUCCESS"
                                );
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap(
                                "table.init: expected i32 values for offsets and size",
                            ));
                        }
                    }

                    Instruction::ElemDrop(elem_seg_idx) => {
                        // elem.drop: [] -> []
                        // Drop (mark as unavailable) an element segment
                        // Note: This operation marks the segment as dropped so it can't be used
                        // by future table.init operations. The actual segment data may still exist
                        // in memory but is logically unavailable.
                        #[cfg(feature = "tracing")]
                        trace!(
                            elem_seg_idx = elem_seg_idx,
                            "[ElemDrop] Dropping element segment"
                        );

                        // Validate element segment index exists
                        #[cfg(feature = "std")]
                        if elem_seg_idx as usize >= module.elements.len() {
                            return Err(wrt_error::Error::runtime_trap(
                                "elem.drop: invalid element segment index",
                            ));
                        }
                        #[cfg(not(feature = "std"))]
                        if module.elements.get(elem_seg_idx as usize).is_err() {
                            return Err(wrt_error::Error::runtime_trap(
                                "elem.drop: invalid element segment index",
                            ));
                        }

                        // Mark the element segment as dropped in the instance
                        // After this, table.init will treat the segment as having 0 length
                        instance.drop_element_segment(elem_seg_idx)?;

                        #[cfg(feature = "tracing")]
                        trace!(
                            elem_seg_idx = elem_seg_idx,
                            "[ElemDrop] SUCCESS (segment marked as dropped)"
                        );
                    }

                    // ===============================================
                    // Atomic Memory Operations (0xFE prefix)
                    // WebAssembly Threads and Atomics proposal
                    // ===============================================

                    // Memory synchronization operations
                    Instruction::MemoryAtomicNotify { memarg } => {
                        // memory.atomic.notify: [i32, i32] -> [i32]
                        // Wake up to count threads waiting on the given address
                        if let (Some(Value::I32(count)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            // Check alignment - notify requires 4-byte alignment
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            #[cfg(feature = "tracing")]
                            trace!(
                                addr = format_args!("0x{:x}", effective_addr),
                                count = count,
                                "[AtomicNotify] Notify operation"
                            );
                            // For single-threaded runtime, notify always returns 0 (no waiters)
                            // A full implementation would use futex-like mechanisms
                            operand_stack.push(Value::I32(0));
                        }
                    }

                    Instruction::MemoryAtomicWait32 { memarg } => {
                        // memory.atomic.wait32: [i32, i32, i64] -> [i32]
                        // Wait for i32 value at address to change, with timeout
                        if let (Some(Value::I64(timeout)), Some(Value::I32(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            // Check alignment - wait32 requires 4-byte alignment
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            #[cfg(feature = "tracing")]
                            trace!(
                                addr = format_args!("0x{:x}", effective_addr),
                                expected = expected,
                                timeout = timeout,
                                "[AtomicWait32] Wait operation"
                            );
                            // For single-threaded runtime, return 1 (not equal) or 2 (timed out)
                            // We'll read the current value and return immediately
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let current = i32::from_le_bytes(buffer);
                                            // Return 1 if value differs, 2 if would timeout (single-threaded)
                                            let result = if current != expected { 1 } else { 2 };
                                            operand_stack.push(Value::I32(result));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::MemoryAtomicWait64 { memarg } => {
                        // memory.atomic.wait64: [i32, i64, i64] -> [i32]
                        // Wait for i64 value at address to change, with timeout
                        if let (Some(Value::I64(timeout)), Some(Value::I64(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            // Check alignment - wait64 requires 8-byte alignment
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            #[cfg(feature = "tracing")]
                            trace!(
                                addr = format_args!("0x{:x}", effective_addr),
                                expected = expected,
                                timeout = timeout,
                                "[AtomicWait64] Wait operation"
                            );
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let current = i64::from_le_bytes(buffer);
                                            let result = if current != expected { 1 } else { 2 };
                                            operand_stack.push(Value::I32(result));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::AtomicFence => {
                        // atomic.fence: [] -> []
                        // Memory fence - ensures ordering of memory operations
                        #[cfg(feature = "tracing")]
                        trace!("[AtomicFence] Memory fence");
                        core::sync::atomic::fence(Ordering::SeqCst);
                    }

                    // ===============================================
                    // Atomic Loads
                    // ===============================================

                    Instruction::I32AtomicLoad { memarg } => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            // Check 4-byte alignment for i32
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let value = i32::from_le_bytes(buffer);
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I32AtomicLoad] Loaded"
                                            );
                                            operand_stack.push(Value::I32(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicLoad { memarg } => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            // Check 8-byte alignment for i64
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let value = i64::from_le_bytes(buffer);
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I64AtomicLoad] Loaded"
                                            );
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicLoad8U { memarg } => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            // 8-bit loads have natural alignment (1 byte)
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let value = buffer[0] as i32;
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I32AtomicLoad8U] Loaded"
                                            );
                                            operand_stack.push(Value::I32(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicLoad16U { memarg } => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            // Check 2-byte alignment for i16
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let value = u16::from_le_bytes(buffer) as i32;
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I32AtomicLoad16U] Loaded"
                                            );
                                            operand_stack.push(Value::I32(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicLoad8U { memarg } => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let value = buffer[0] as i64;
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I64AtomicLoad8U] Loaded"
                                            );
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicLoad16U { memarg } => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let value = u16::from_le_bytes(buffer) as i64;
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I64AtomicLoad16U] Loaded"
                                            );
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicLoad32U { memarg } => {
                        if let Some(Value::I32(addr)) = operand_stack.pop() {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let value = u32::from_le_bytes(buffer) as i64;
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I64AtomicLoad32U] Loaded"
                                            );
                                            operand_stack.push(Value::I64(value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // ===============================================
                    // Atomic Stores
                    // ===============================================

                    Instruction::I32AtomicStore { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = value.to_le_bytes();
                                    match memory.write_shared(effective_addr, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I32AtomicStore] Stored"
                                            );
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicStore { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = value.to_le_bytes();
                                    match memory.write_shared(effective_addr, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value,
                                                "[I64AtomicStore] Stored"
                                            );
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicStore8 { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = [(value as u8)];
                                    match memory.write_shared(effective_addr, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value as u8,
                                                "[I32AtomicStore8] Stored"
                                            );
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicStore16 { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = (value as u16).to_le_bytes();
                                    match memory.write_shared(effective_addr, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value as u16,
                                                "[I32AtomicStore16] Stored"
                                            );
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicStore8 { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = [(value as u8)];
                                    match memory.write_shared(effective_addr, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value as u8,
                                                "[I64AtomicStore8] Stored"
                                            );
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicStore16 { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = (value as u16).to_le_bytes();
                                    match memory.write_shared(effective_addr, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value as u16,
                                                "[I64AtomicStore16] Stored"
                                            );
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicStore32 { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let bytes = (value as u32).to_le_bytes();
                                    match memory.write_shared(effective_addr, &bytes) {
                                        Ok(()) => {
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                value = value as u32,
                                                "[I64AtomicStore32] Stored"
                                            );
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // ===============================================
                    // Atomic RMW Add Operations
                    // ===============================================

                    Instruction::I32AtomicRmwAdd { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i32::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_add(value);
                                            let new_bytes = new_value.to_le_bytes();
                                            match memory.write_shared(effective_addr, &new_bytes) {
                                                Ok(()) => {
                                                    #[cfg(feature = "tracing")]
                                                    trace!(
                                                        addr = format_args!("0x{:x}", effective_addr),
                                                        old_value = old_value,
                                                        add_value = value,
                                                        new_value = new_value,
                                                        "[I32AtomicRmwAdd] RMW Add"
                                                    );
                                                    operand_stack.push(Value::I32(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmwAdd { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i64::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_add(value);
                                            let new_bytes = new_value.to_le_bytes();
                                            match memory.write_shared(effective_addr, &new_bytes) {
                                                Ok(()) => {
                                                    #[cfg(feature = "tracing")]
                                                    trace!(
                                                        addr = format_args!("0x{:x}", effective_addr),
                                                        old_value = old_value,
                                                        "[I64AtomicRmwAdd] RMW Add"
                                                    );
                                                    operand_stack.push(Value::I64(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw8AddU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value.wrapping_add(value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw16AddU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_add(value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw8AddU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value.wrapping_add(value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw16AddU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_add(value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw32AddU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u32::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_add(value as u32);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // ===============================================
                    // Atomic RMW Sub Operations
                    // ===============================================

                    Instruction::I32AtomicRmwSub { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i32::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_sub(value);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmwSub { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i64::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_sub(value);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw8SubU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value.wrapping_sub(value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw16SubU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_sub(value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw8SubU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value.wrapping_sub(value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw16SubU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_sub(value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw32SubU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u32::from_le_bytes(buffer);
                                            let new_value = old_value.wrapping_sub(value as u32);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // ===============================================
                    // Atomic RMW And Operations
                    // ===============================================

                    Instruction::I32AtomicRmwAnd { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i32::from_le_bytes(buffer);
                                            let new_value = old_value & value;
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmwAnd { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i64::from_le_bytes(buffer);
                                            let new_value = old_value & value;
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw8AndU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value & (value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw16AndU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value & (value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw8AndU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value & (value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw16AndU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value & (value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw32AndU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u32::from_le_bytes(buffer);
                                            let new_value = old_value & (value as u32);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // ===============================================
                    // Atomic RMW Or Operations
                    // ===============================================

                    Instruction::I32AtomicRmwOr { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i32::from_le_bytes(buffer);
                                            let new_value = old_value | value;
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmwOr { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i64::from_le_bytes(buffer);
                                            let new_value = old_value | value;
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw8OrU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value | (value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw16OrU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value | (value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw8OrU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value | (value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw16OrU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value | (value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw32OrU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u32::from_le_bytes(buffer);
                                            let new_value = old_value | (value as u32);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // ===============================================
                    // Atomic RMW Xor Operations
                    // ===============================================

                    Instruction::I32AtomicRmwXor { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i32::from_le_bytes(buffer);
                                            let new_value = old_value ^ value;
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmwXor { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i64::from_le_bytes(buffer);
                                            let new_value = old_value ^ value;
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw8XorU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value ^ (value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw16XorU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value ^ (value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw8XorU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            let new_value = old_value ^ (value as u8);
                                            match memory.write_shared(effective_addr, &[new_value]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw16XorU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            let new_value = old_value ^ (value as u16);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw32XorU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u32::from_le_bytes(buffer);
                                            let new_value = old_value ^ (value as u32);
                                            match memory.write_shared(effective_addr, &new_value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // ===============================================
                    // Atomic RMW Exchange Operations
                    // ===============================================

                    Instruction::I32AtomicRmwXchg { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i32::from_le_bytes(buffer);
                                            match memory.write_shared(effective_addr, &value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmwXchg { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i64::from_le_bytes(buffer);
                                            match memory.write_shared(effective_addr, &value.to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw8XchgU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            match memory.write_shared(effective_addr, &[(value as u8)]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw16XchgU { memarg } => {
                        if let (Some(Value::I32(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            match memory.write_shared(effective_addr, &(value as u16).to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I32(old_value as i32));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw8XchgU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            match memory.write_shared(effective_addr, &[(value as u8)]) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw16XchgU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            match memory.write_shared(effective_addr, &(value as u16).to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw32XchgU { memarg } => {
                        if let (Some(Value::I64(value)), Some(Value::I32(addr))) = (operand_stack.pop(), operand_stack.pop()) {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u32::from_le_bytes(buffer);
                                            match memory.write_shared(effective_addr, &(value as u32).to_le_bytes()) {
                                                Ok(()) => {
                                                    operand_stack.push(Value::I64(old_value as i64));
                                                }
                                                Err(_) => {
                                                    return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // ===============================================
                    // Atomic Compare-Exchange Operations
                    // ===============================================

                    Instruction::I32AtomicRmwCmpxchg { memarg } => {
                        // cmpxchg: [addr, expected, replacement] -> [old_value]
                        if let (Some(Value::I32(replacement)), Some(Value::I32(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i32::from_le_bytes(buffer);
                                            // Only write if old_value == expected
                                            if old_value == expected {
                                                match memory.write_shared(effective_addr, &replacement.to_le_bytes()) {
                                                    Ok(()) => {}
                                                    Err(_) => {
                                                        return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                    }
                                                }
                                            }
                                            #[cfg(feature = "tracing")]
                                            trace!(
                                                addr = format_args!("0x{:x}", effective_addr),
                                                old_value = old_value,
                                                expected = expected,
                                                replacement = replacement,
                                                swapped = (old_value == expected),
                                                "[I32AtomicRmwCmpxchg] CmpXchg"
                                            );
                                            operand_stack.push(Value::I32(old_value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmwCmpxchg { memarg } => {
                        if let (Some(Value::I64(replacement)), Some(Value::I64(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 8 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 8];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = i64::from_le_bytes(buffer);
                                            if old_value == expected {
                                                match memory.write_shared(effective_addr, &replacement.to_le_bytes()) {
                                                    Ok(()) => {}
                                                    Err(_) => {
                                                        return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                    }
                                                }
                                            }
                                            operand_stack.push(Value::I64(old_value));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw8CmpxchgU { memarg } => {
                        if let (Some(Value::I32(replacement)), Some(Value::I32(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            if old_value == (expected as u8) {
                                                match memory.write_shared(effective_addr, &[(replacement as u8)]) {
                                                    Ok(()) => {}
                                                    Err(_) => {
                                                        return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                    }
                                                }
                                            }
                                            operand_stack.push(Value::I32(old_value as i32));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I32AtomicRmw16CmpxchgU { memarg } => {
                        if let (Some(Value::I32(replacement)), Some(Value::I32(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            if old_value == (expected as u16) {
                                                match memory.write_shared(effective_addr, &(replacement as u16).to_le_bytes()) {
                                                    Ok(()) => {}
                                                    Err(_) => {
                                                        return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                    }
                                                }
                                            }
                                            operand_stack.push(Value::I32(old_value as i32));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw8CmpxchgU { memarg } => {
                        if let (Some(Value::I64(replacement)), Some(Value::I64(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 1];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = buffer[0];
                                            if old_value == (expected as u8) {
                                                match memory.write_shared(effective_addr, &[(replacement as u8)]) {
                                                    Ok(()) => {}
                                                    Err(_) => {
                                                        return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                    }
                                                }
                                            }
                                            operand_stack.push(Value::I64(old_value as i64));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw16CmpxchgU { memarg } => {
                        if let (Some(Value::I64(replacement)), Some(Value::I64(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 2 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 2];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u16::from_le_bytes(buffer);
                                            if old_value == (expected as u16) {
                                                match memory.write_shared(effective_addr, &(replacement as u16).to_le_bytes()) {
                                                    Ok(()) => {}
                                                    Err(_) => {
                                                        return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                    }
                                                }
                                            }
                                            operand_stack.push(Value::I64(old_value as i64));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    Instruction::I64AtomicRmw32CmpxchgU { memarg } => {
                        if let (Some(Value::I64(replacement)), Some(Value::I64(expected)), Some(Value::I32(addr))) =
                            (operand_stack.pop(), operand_stack.pop(), operand_stack.pop())
                        {
                            let effective_addr = (addr as u32).wrapping_add(memarg.offset);
                            if effective_addr % 4 != 0 {
                                return Err(wrt_error::Error::runtime_trap("unaligned atomic access"));
                            }
                            match instance.memory(memarg.memory_index as u32) {
                                Ok(memory_wrapper) => {
                                    let memory = &memory_wrapper.0;
                                    let mut buffer = [0u8; 4];
                                    match memory.read(effective_addr, &mut buffer) {
                                        Ok(()) => {
                                            let old_value = u32::from_le_bytes(buffer);
                                            if old_value == (expected as u32) {
                                                match memory.write_shared(effective_addr, &(replacement as u32).to_le_bytes()) {
                                                    Ok(()) => {}
                                                    Err(_) => {
                                                        return Err(wrt_error::Error::runtime_trap("Memory write out of bounds"));
                                                    }
                                                }
                                            }
                                            operand_stack.push(Value::I64(old_value as i64));
                                        }
                                        Err(_) => {
                                            return Err(wrt_error::Error::runtime_trap("Memory read out of bounds"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(wrt_error::Error::runtime_trap("Memory access error"));
                                }
                            }
                        }
                    }

                    // End of atomic operations
                    // ===============================================

                    // ========================================
                    // Exception Handling Instructions
                    // ========================================
                    Instruction::Try { block_type_idx } => {
                        // Try block - similar to Block but with exception handling
                        // For Phase 4, we just push a "try" frame; full handler logic comes in Phase 5
                        block_depth += 1;
                        block_stack.push(("try", pc, block_type_idx, operand_stack.len()));
                        #[cfg(feature = "tracing")]
                        trace!("Try: block_type_idx={}, depth now {}, stack_height={}", block_type_idx, block_depth, operand_stack.len());
                        // Continue execution - End will close this like any other block
                    }
                    Instruction::Throw(tag_idx) => {
                        // Throw an exception with the given tag
                        #[cfg(feature = "tracing")]
                        trace!(
                            tag_idx = tag_idx,
                            pc = pc,
                            func_idx = func_idx,
                            block_stack_len = block_stack.len(),
                            tags_len = module.tags.len(),
                            "[EXCEPTION] Throw instruction"
                        );

                        // Get the exception payload values by looking up the tag's type
                        // Tag's type_idx points to a FuncType whose params are the exception values
                        // Use get_tag_type to handle both imported and defined tags
                        let mut exception_payload: Vec<Value> = Vec::new();
                        if let Some(tag_type) = module.get_tag_type(tag_idx) {
                            if let Some(func_type) = module.types.get(tag_type.type_idx as usize) {
                                // Pop values in reverse order (last param is top of stack)
                                for _ in 0..func_type.params.len() {
                                    if let Some(val) = operand_stack.pop() {
                                        exception_payload.push(val);
                                    }
                                }
                                // Reverse to get correct order (first param first)
                                exception_payload.reverse();
                                #[cfg(feature = "tracing")]
                                trace!(
                                    payload_len = exception_payload.len(),
                                    "[EXCEPTION] Captured exception payload"
                                );
                            }
                        }

                        // Get the effective tag identity for identity-based matching
                        // This handles both imported tags and exported local tags
                        let thrown_tag_identity = self.get_effective_tag_identity(instance_id, &module, tag_idx);

                        // Walk block_stack from top to bottom looking for try_table with matching handler
                        let mut found_handler = false;
                        let mut handler_label = 0u32;
                        let mut handler_is_ref = false;
                        let mut try_block_idx = 0usize;

                        for (idx, (block_type, try_pc, _, _)) in block_stack.iter().enumerate().rev() {
                            if *block_type == "try_table" {
                                // Found a try_table - check its handlers
                                if let Some(try_instr) = instructions.get(*try_pc) {
                                    if let Instruction::TryTable { handlers, .. } = try_instr {
                                        // Check each handler for a match
                                        for i in 0..handlers.len() {
                                            if let Ok(handler) = handlers.get(i) {
                                                let handler_val = handler.clone();
                                                let (matches, lbl, is_ref) = match handler_val {
                                                    wrt_foundation::types::CatchHandler::Catch { tag_idx: htag, label: hlbl } => {
                                                        // Get handler tag's effective identity for comparison
                                                        let handler_identity = self.get_effective_tag_identity(instance_id, &module, htag);
                                                        // Match by identity if both have identity, by index if both local-only
                                                        let tag_matches = match (&thrown_tag_identity, &handler_identity) {
                                                            (Some(ex_id), Some(h_id)) => ex_id == h_id,
                                                            (None, None) => htag == tag_idx,
                                                            _ => false,
                                                        };
                                                        (tag_matches, hlbl, false)
                                                    }
                                                    wrt_foundation::types::CatchHandler::CatchRef { tag_idx: htag, label: hlbl } => {
                                                        // Get handler tag's effective identity for comparison
                                                        let handler_identity = self.get_effective_tag_identity(instance_id, &module, htag);
                                                        // Match by identity if both have identity, by index if both local-only
                                                        let tag_matches = match (&thrown_tag_identity, &handler_identity) {
                                                            (Some(ex_id), Some(h_id)) => ex_id == h_id,
                                                            (None, None) => htag == tag_idx,
                                                            _ => false,
                                                        };
                                                        (tag_matches, hlbl, true)
                                                    }
                                                    wrt_foundation::types::CatchHandler::CatchAll { label } => {
                                                        (true, label, false)
                                                    }
                                                    wrt_foundation::types::CatchHandler::CatchAllRef { label } => {
                                                        (true, label, true)
                                                    }
                                                };
                                                if matches {
                                                    handler_label = lbl;
                                                    handler_is_ref = is_ref;
                                                    found_handler = true;
                                                    try_block_idx = idx;
                                                    break;
                                                }
                                            }
                                        }
                                        if found_handler {
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        if found_handler {
                            #[cfg(feature = "tracing")]
                            trace!(
                                handler_label = handler_label,
                                handler_is_ref = handler_is_ref,
                                try_block_idx = try_block_idx,
                                "[EXCEPTION] Found handler, branching"
                            );

                            // Branch to handler label - need to find the End of the target block
                            // Per wast crate encoding, handler_label is relative to try_table's PARENT:
                            // - handler_label=0 means branch to try_table's parent (containing block)
                            // - handler_label=N means branch N levels further out from parent
                            // Calculate target block index BEFORE popping
                            // target = try_block_idx - 1 (parent) - handler_label (additional levels)
                            let target_block_idx = if handler_label as usize + 1 <= try_block_idx {
                                try_block_idx - 1 - handler_label as usize
                            } else {
                                // Label targets function level - return with exception
                                #[cfg(feature = "tracing")]
                                trace!("[EXCEPTION] Handler label {} exceeds block depth, returning", handler_label);
                                break;
                            };

                            // Get entry stack height before popping blocks
                            let (_, _, _, entry_stack_height) = block_stack[target_block_idx];
                            // Save original block count for depth calculation
                            let original_block_count = block_stack.len();

                            // Pop blocks from stack down to (but not including) target block
                            // The target block will be popped when we execute its End instruction
                            while block_stack.len() > target_block_idx + 1 {
                                block_stack.pop();
                                block_depth -= 1;
                            }

                            // Clear stack to entry height
                            while operand_stack.len() > entry_stack_height {
                                operand_stack.pop();
                            }

                            // Push exception payload values to stack (for catch handlers)
                            for val in exception_payload.iter() {
                                operand_stack.push(val.clone());
                            }

                            // If handler is *Ref variant, store exception and push exnref
                            if handler_is_ref {
                                // Store exception in exception_storage for throw_ref
                                let exn_idx = self.exception_storage.len() as u32;
                                self.exception_storage.push((tag_idx, thrown_tag_identity.clone(), exception_payload.clone()));
                                operand_stack.push(Value::ExnRef(Some(exn_idx)));
                            }

                            #[cfg(feature = "tracing")]
                            trace!(
                                stack_len = operand_stack.len(),
                                payload_pushed = exception_payload.len(),
                                target_block_idx = target_block_idx,
                                block_stack_len = block_stack.len(),
                                "[EXCEPTION] Stack restored with payload"
                            );

                            // Find the End of the target block by scanning forward
                            // We need to skip over ALL inner blocks' Ends (not just try_table's)
                            // depth = number of blocks we're exiting from throw location to target
                            let mut ends_to_find = 1i32;  // Just looking for target block's End
                            let mut new_pc = pc + 1;
                            // Skip all blocks from throw location down to (but not including) target
                            // Use original_block_count (before popping) to know how many ends to skip
                            let mut depth = (original_block_count - 1 - target_block_idx) as i32;

                            while new_pc < instructions.len() && ends_to_find > 0 {
                                if let Some(instr) = instructions.get(new_pc) {
                                    match instr {
                                        Instruction::Block { .. } |
                                        Instruction::Loop { .. } |
                                        Instruction::If { .. } |
                                        Instruction::Try { .. } |
                                        Instruction::TryTable { .. } => depth += 1,
                                        Instruction::End => {
                                            if depth == 0 {
                                                ends_to_find -= 1;
                                                if ends_to_find == 0 {
                                                    // Found the End of target block
                                                    // Set pc to this End so it executes next
                                                    pc = new_pc - 1;
                                                    break;
                                                }
                                            } else {
                                                depth -= 1;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                new_pc += 1;
                            }
                        } else {
                            // No handler found in current function - store exception state for propagation
                            #[cfg(feature = "std")]
                            {
                                // Get effective tag identity for cross-module exception matching
                                // This handles both imported tags and exported local tags
                                let tag_identity = self.get_effective_tag_identity(instance_id, &module, tag_idx);
                                self.active_exception = Some((instance_id, tag_idx, tag_identity, exception_payload));
                            }
                            #[cfg(feature = "tracing")]
                            trace!(
                                tag_idx = tag_idx,
                                "[EXCEPTION] No handler found - propagating to caller"
                            );
                            // Return error to trigger unwinding - caller will check active_exception
                            return Err(wrt_error::Error::runtime_trap(
                                "exception",
                            ));
                        }
                    }
                    Instruction::ThrowRef => {
                        // Throw an exception from an exnref on the stack
                        // Pop the exnref and re-throw with original tag and payload
                        if let Some(ref_val) = operand_stack.pop() {
                            let (tag_idx, thrown_tag_identity, exception_payload) = match ref_val {
                                Value::ExnRef(Some(exn_idx)) => {
                                    // Look up the exception from storage (includes tag identity)
                                    if let Some((stored_tag, stored_identity, stored_payload)) = self.exception_storage.get(exn_idx as usize) {
                                        (*stored_tag, stored_identity.clone(), stored_payload.clone())
                                    } else {
                                        return Err(wrt_error::Error::runtime_trap(
                                            "invalid exception reference",
                                        ));
                                    }
                                }
                                Value::ExnRef(None) => {
                                    // Throwing null exnref is a trap
                                    return Err(wrt_error::Error::runtime_trap(
                                        "null exception reference",
                                    ));
                                }
                                _ => {
                                    return Err(wrt_error::Error::runtime_type_mismatch(
                                        "throw_ref expects an exnref",
                                    ));
                                }
                            };

                            #[cfg(feature = "tracing")]
                            trace!(
                                tag_idx = tag_idx,
                                payload_len = exception_payload.len(),
                                pc = pc,
                                func_idx = func_idx,
                                "[EXCEPTION] ThrowRef re-throwing exception"
                            );

                            // Now do the same handler search as Throw instruction
                            let mut found_handler = false;
                            let mut handler_label = 0u32;
                            let mut handler_is_ref = false;
                            let mut try_block_idx = 0usize;

                            for (idx, (block_type, try_pc, _, _)) in block_stack.iter().enumerate().rev() {
                                if *block_type == "try_table" {
                                    if let Some(try_instr) = instructions.get(*try_pc) {
                                        if let Instruction::TryTable { handlers, .. } = try_instr {
                                            for i in 0..handlers.len() {
                                                if let Ok(handler) = handlers.get(i) {
                                                    let handler_val = handler.clone();
                                                    let (matches, lbl, is_ref) = match handler_val {
                                                        wrt_foundation::types::CatchHandler::Catch { tag_idx: htag, label: hlbl } => {
                                                            // Get handler tag's effective identity for comparison
                                                            let handler_identity = self.get_effective_tag_identity(instance_id, &module, htag);
                                                            // Match by identity if both have identity, by index if both local-only
                                                            let tag_matches = match (&thrown_tag_identity, &handler_identity) {
                                                                (Some(ex_id), Some(h_id)) => ex_id == h_id,
                                                                (None, None) => htag == tag_idx,
                                                                _ => false,
                                                            };
                                                            (tag_matches, hlbl, false)
                                                        }
                                                        wrt_foundation::types::CatchHandler::CatchRef { tag_idx: htag, label: hlbl } => {
                                                            // Get handler tag's effective identity for comparison
                                                            let handler_identity = self.get_effective_tag_identity(instance_id, &module, htag);
                                                            // Match by identity if both have identity, by index if both local-only
                                                            let tag_matches = match (&thrown_tag_identity, &handler_identity) {
                                                                (Some(ex_id), Some(h_id)) => ex_id == h_id,
                                                                (None, None) => htag == tag_idx,
                                                                _ => false,
                                                            };
                                                            (tag_matches, hlbl, true)
                                                        }
                                                        wrt_foundation::types::CatchHandler::CatchAll { label } => {
                                                            (true, label, false)
                                                        }
                                                        wrt_foundation::types::CatchHandler::CatchAllRef { label } => {
                                                            (true, label, true)
                                                        }
                                                    };
                                                    if matches {
                                                        handler_label = lbl;
                                                        handler_is_ref = is_ref;
                                                        found_handler = true;
                                                        try_block_idx = idx;
                                                        break;
                                                    }
                                                }
                                            }
                                            if found_handler {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            if found_handler {
                                // Calculate target block index BEFORE popping
                                // Per wast crate encoding, handler_label is relative to try_table's PARENT
                                let target_block_idx = if handler_label as usize + 1 <= try_block_idx {
                                    try_block_idx - 1 - handler_label as usize
                                } else {
                                    break; // Function return
                                };

                                let (_, _, _, entry_stack_height) = block_stack[target_block_idx];
                                // Save original block count for depth calculation
                                let original_block_count = block_stack.len();

                                // Pop blocks down to target
                                while block_stack.len() > target_block_idx + 1 {
                                    block_stack.pop();
                                    block_depth -= 1;
                                }

                                // Clear stack to entry height
                                while operand_stack.len() > entry_stack_height {
                                    operand_stack.pop();
                                }

                                // Push exception payload
                                for val in exception_payload.iter() {
                                    operand_stack.push(val.clone());
                                }

                                // If handler is *Ref variant, store and push new exnref
                                if handler_is_ref {
                                    let new_exn_idx = self.exception_storage.len() as u32;
                                    self.exception_storage.push((tag_idx, thrown_tag_identity.clone(), exception_payload.clone()));
                                    operand_stack.push(Value::ExnRef(Some(new_exn_idx)));
                                }

                                // Find End of target block
                                let mut ends_to_find = 1i32;  // Just looking for target block's End
                                let mut new_pc = pc + 1;
                                // Skip all blocks from throw location down to (but not including) target
                                let mut depth = (original_block_count - 1 - target_block_idx) as i32;

                                while new_pc < instructions.len() && ends_to_find > 0 {
                                    if let Some(instr) = instructions.get(new_pc) {
                                        match instr {
                                            Instruction::Block { .. } |
                                            Instruction::Loop { .. } |
                                            Instruction::If { .. } |
                                            Instruction::Try { .. } |
                                            Instruction::TryTable { .. } => depth += 1,
                                            Instruction::End => {
                                                if depth == 0 {
                                                    ends_to_find -= 1;
                                                    if ends_to_find == 0 {
                                                        pc = new_pc - 1;
                                                        break;
                                                    }
                                                } else {
                                                    depth -= 1;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    new_pc += 1;
                                }
                            } else {
                                // No handler - propagate to caller
                                #[cfg(feature = "std")]
                                {
                                    // Use the stored tag identity for cross-module exception matching
                                    // (thrown_tag_identity was retrieved from exception_storage)
                                    self.active_exception = Some((instance_id, tag_idx, thrown_tag_identity, exception_payload));
                                }
                                return Err(wrt_error::Error::runtime_trap("exception"));
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap(
                                "throw_ref: stack underflow",
                            ));
                        }
                    }
                    Instruction::TryTable { block_type_idx, .. } => {
                        // Modern try_table block with catch handlers
                        // For Phase 4, we treat it like a regular block
                        // Full handler matching comes in Phase 5
                        block_depth += 1;
                        block_stack.push(("try_table", pc, block_type_idx, operand_stack.len()));
                        #[cfg(feature = "tracing")]
                        trace!("TryTable: block_type_idx={}, depth now {}, stack_height={}", block_type_idx, block_depth, operand_stack.len());
                    }
                    Instruction::Catch(_tag_idx) => {
                        // Legacy catch clause - skip forward to find End
                        // This is only reached if we fall through from try block
                        // (no exception was thrown), so we skip the catch code
                        #[cfg(feature = "tracing")]
                        trace!("Catch: skipping catch clause (no exception), tag_idx={}", _tag_idx);

                        // Scan forward to find matching End, skipping nested blocks
                        let mut depth = 1;
                        let mut new_pc = pc + 1;
                        while new_pc < instructions.len() && depth > 0 {
                            if let Some(instr) = instructions.get(new_pc) {
                                match instr {
                                    Instruction::Block { .. } |
                                    Instruction::Loop { .. } |
                                    Instruction::If { .. } |
                                    Instruction::Try { .. } |
                                    Instruction::TryTable { .. } => depth += 1,
                                    Instruction::End => depth -= 1,
                                    _ => {}
                                }
                            }
                            if depth > 0 { new_pc += 1; }
                        }
                        pc = new_pc - 1; // -1 because we'll +1 at end of loop
                    }
                    Instruction::CatchAll => {
                        // Legacy catch_all clause - skip forward to End
                        #[cfg(feature = "tracing")]
                        trace!("CatchAll: skipping catch_all clause (no exception)");

                        let mut depth = 1;
                        let mut new_pc = pc + 1;
                        while new_pc < instructions.len() && depth > 0 {
                            if let Some(instr) = instructions.get(new_pc) {
                                match instr {
                                    Instruction::Block { .. } |
                                    Instruction::Loop { .. } |
                                    Instruction::If { .. } |
                                    Instruction::Try { .. } |
                                    Instruction::TryTable { .. } => depth += 1,
                                    Instruction::End => depth -= 1,
                                    _ => {}
                                }
                            }
                            if depth > 0 { new_pc += 1; }
                        }
                        pc = new_pc - 1;
                    }
                    Instruction::Rethrow(label_idx) => {
                        // Legacy rethrow - rethrow exception from a catch block
                        // For Phase 4, just trap
                        #[cfg(feature = "tracing")]
                        error!(
                            label_idx = label_idx,
                            pc = pc,
                            "[EXCEPTION] Rethrow instruction"
                        );
                        return Err(wrt_error::Error::runtime_trap(
                            "uncaught exception (rethrow)",
                        ));
                    }
                    Instruction::Delegate(label_idx) => {
                        // Legacy delegate - delegate exception to outer handler
                        // For Phase 4, treat like branch to end of block
                        #[cfg(feature = "tracing")]
                        trace!("Delegate: delegating to label_idx={}", label_idx);

                        // Pop blocks and branch to the target's End
                        if (label_idx as usize) < block_stack.len() {
                            let blocks_to_pop = label_idx as usize;
                            for _ in 0..blocks_to_pop {
                                if !block_stack.is_empty() {
                                    block_stack.pop();
                                    block_depth -= 1;
                                }
                            }

                            // Scan forward to find the End
                            let mut target_depth = (label_idx as i32) + 1;
                            let mut new_pc = pc + 1;
                            let mut depth = 0;

                            while new_pc < instructions.len() && target_depth > 0 {
                                if let Some(instr) = instructions.get(new_pc) {
                                    match instr {
                                        Instruction::Block { .. } |
                                        Instruction::Loop { .. } |
                                        Instruction::If { .. } |
                                        Instruction::Try { .. } |
                                        Instruction::TryTable { .. } => depth += 1,
                                        Instruction::End => {
                                            if depth == 0 {
                                                target_depth -= 1;
                                                if target_depth == 0 {
                                                    pc = new_pc - 1;
                                                    break;
                                                }
                                            } else {
                                                depth -= 1;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                new_pc += 1;
                            }
                        }
                    }

                    // ========================================
                    // GC (Garbage Collection) Instructions
                    // WebAssembly GC Proposal
                    // ========================================

                    Instruction::StructNew(type_idx) => {
                        // struct.new: [field_values...] -> [structref]
                        // Pop field values, create struct, push reference
                        #[cfg(feature = "tracing")]
                        trace!("StructNew: type_idx={}", type_idx);
                        // For now, create an empty struct reference
                        // Full implementation requires type info to pop correct number of fields
                        let struct_ref = wrt_foundation::values::StructRef::new(
                            type_idx,
                            wrt_foundation::traits::DefaultMemoryProvider::default()
                        ).map_err(|_| wrt_error::Error::runtime_error("Failed to create struct"))?;
                        operand_stack.push(Value::StructRef(Some(struct_ref)));
                    }

                    Instruction::StructNewDefault(type_idx) => {
                        // struct.new_default: [] -> [structref]
                        // Create struct with default field values
                        #[cfg(feature = "tracing")]
                        trace!("StructNewDefault: type_idx={}", type_idx);
                        let struct_ref = wrt_foundation::values::StructRef::new(
                            type_idx,
                            wrt_foundation::traits::DefaultMemoryProvider::default()
                        ).map_err(|_| wrt_error::Error::runtime_error("Failed to create struct"))?;
                        operand_stack.push(Value::StructRef(Some(struct_ref)));
                    }

                    Instruction::StructGet(type_idx, field_idx) => {
                        // struct.get: [structref] -> [value]
                        #[cfg(feature = "tracing")]
                        trace!("StructGet: type_idx={}, field_idx={}", type_idx, field_idx);
                        if let Some(Value::StructRef(Some(s))) = operand_stack.pop() {
                            if let Ok(field) = s.get_field(field_idx as usize) {
                                operand_stack.push(field.clone());
                            } else {
                                return Err(wrt_error::Error::runtime_trap("struct.get: field index out of bounds"));
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap("struct.get: null reference"));
                        }
                    }

                    Instruction::StructGetS(type_idx, field_idx) => {
                        // struct.get_s: [structref] -> [i32] (sign-extend packed field)
                        #[cfg(feature = "tracing")]
                        trace!("StructGetS: type_idx={}, field_idx={}", type_idx, field_idx);
                        if let Some(Value::StructRef(Some(s))) = operand_stack.pop() {
                            if let Ok(field) = s.get_field(field_idx as usize) {
                                operand_stack.push(field.clone());
                            } else {
                                return Err(wrt_error::Error::runtime_trap("struct.get_s: field index out of bounds"));
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap("struct.get_s: null reference"));
                        }
                    }

                    Instruction::StructGetU(type_idx, field_idx) => {
                        // struct.get_u: [structref] -> [i32] (zero-extend packed field)
                        #[cfg(feature = "tracing")]
                        trace!("StructGetU: type_idx={}, field_idx={}", type_idx, field_idx);
                        if let Some(Value::StructRef(Some(s))) = operand_stack.pop() {
                            if let Ok(field) = s.get_field(field_idx as usize) {
                                operand_stack.push(field.clone());
                            } else {
                                return Err(wrt_error::Error::runtime_trap("struct.get_u: field index out of bounds"));
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap("struct.get_u: null reference"));
                        }
                    }

                    Instruction::StructSet(type_idx, field_idx) => {
                        // struct.set: [structref value] -> []
                        #[cfg(feature = "tracing")]
                        trace!("StructSet: type_idx={}, field_idx={}", type_idx, field_idx);
                        let value = operand_stack.pop().ok_or_else(||
                            wrt_error::Error::runtime_trap("struct.set: expected value"))?;
                        if let Some(Value::StructRef(Some(mut s))) = operand_stack.pop() {
                            s.set_field(field_idx as usize, value).map_err(|_|
                                wrt_error::Error::runtime_trap("struct.set: field index out of bounds"))?;
                        } else {
                            return Err(wrt_error::Error::runtime_trap("struct.set: null reference"));
                        }
                    }

                    Instruction::ArrayNew(type_idx) => {
                        // array.new: [value i32] -> [arrayref]
                        #[cfg(feature = "tracing")]
                        trace!("ArrayNew: type_idx={}", type_idx);
                        let length = match operand_stack.pop() {
                            Some(Value::I32(n)) => n as u32,
                            _ => return Err(wrt_error::Error::runtime_trap("array.new: expected i32 length")),
                        };
                        let init_value = operand_stack.pop().ok_or_else(||
                            wrt_error::Error::runtime_trap("array.new: expected init value"))?;
                        let mut array_ref = wrt_foundation::values::ArrayRef::new(
                            type_idx,
                            wrt_foundation::traits::DefaultMemoryProvider::default()
                        ).map_err(|_| wrt_error::Error::runtime_error("Failed to create array"))?;
                        for _ in 0..length {
                            array_ref.push(init_value.clone()).map_err(|_|
                                wrt_error::Error::runtime_error("Failed to push to array"))?;
                        }
                        operand_stack.push(Value::ArrayRef(Some(array_ref)));
                    }

                    Instruction::ArrayNewDefault(type_idx) => {
                        // array.new_default: [i32] -> [arrayref]
                        #[cfg(feature = "tracing")]
                        trace!("ArrayNewDefault: type_idx={}", type_idx);
                        let length = match operand_stack.pop() {
                            Some(Value::I32(n)) => n as u32,
                            _ => return Err(wrt_error::Error::runtime_trap("array.new_default: expected i32 length")),
                        };
                        let mut array_ref = wrt_foundation::values::ArrayRef::new(
                            type_idx,
                            wrt_foundation::traits::DefaultMemoryProvider::default()
                        ).map_err(|_| wrt_error::Error::runtime_error("Failed to create array"))?;
                        for _ in 0..length {
                            array_ref.push(Value::I32(0)).map_err(|_|
                                wrt_error::Error::runtime_error("Failed to push to array"))?;
                        }
                        operand_stack.push(Value::ArrayRef(Some(array_ref)));
                    }

                    Instruction::ArrayNewFixed(type_idx, count) => {
                        // array.new_fixed: [values...] -> [arrayref]
                        #[cfg(feature = "tracing")]
                        trace!("ArrayNewFixed: type_idx={}, count={}", type_idx, count);
                        let mut values = Vec::new();
                        for _ in 0..count {
                            if let Some(v) = operand_stack.pop() {
                                values.push(v);
                            }
                        }
                        values.reverse();
                        let mut array_ref = wrt_foundation::values::ArrayRef::new(
                            type_idx,
                            wrt_foundation::traits::DefaultMemoryProvider::default()
                        ).map_err(|_| wrt_error::Error::runtime_error("Failed to create array"))?;
                        for v in values {
                            array_ref.push(v).map_err(|_|
                                wrt_error::Error::runtime_error("Failed to push to array"))?;
                        }
                        operand_stack.push(Value::ArrayRef(Some(array_ref)));
                    }

                    Instruction::ArrayGet(type_idx) => {
                        // array.get: [arrayref i32] -> [value]
                        #[cfg(feature = "tracing")]
                        trace!("ArrayGet: type_idx={}", type_idx);
                        let index = match operand_stack.pop() {
                            Some(Value::I32(n)) => n as usize,
                            _ => return Err(wrt_error::Error::runtime_trap("array.get: expected i32 index")),
                        };
                        if let Some(Value::ArrayRef(Some(a))) = operand_stack.pop() {
                            if let Ok(elem) = a.get(index) {
                                operand_stack.push(elem.clone());
                            } else {
                                return Err(wrt_error::Error::runtime_trap("array.get: index out of bounds"));
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap("array.get: null reference"));
                        }
                    }

                    Instruction::ArrayGetS(type_idx) => {
                        // array.get_s: [arrayref i32] -> [i32] (sign-extend)
                        #[cfg(feature = "tracing")]
                        trace!("ArrayGetS: type_idx={}", type_idx);
                        let index = match operand_stack.pop() {
                            Some(Value::I32(n)) => n as usize,
                            _ => return Err(wrt_error::Error::runtime_trap("array.get_s: expected i32 index")),
                        };
                        if let Some(Value::ArrayRef(Some(a))) = operand_stack.pop() {
                            if let Ok(elem) = a.get(index) {
                                operand_stack.push(elem.clone());
                            } else {
                                return Err(wrt_error::Error::runtime_trap("array.get_s: index out of bounds"));
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap("array.get_s: null reference"));
                        }
                    }

                    Instruction::ArrayGetU(type_idx) => {
                        // array.get_u: [arrayref i32] -> [i32] (zero-extend)
                        #[cfg(feature = "tracing")]
                        trace!("ArrayGetU: type_idx={}", type_idx);
                        let index = match operand_stack.pop() {
                            Some(Value::I32(n)) => n as usize,
                            _ => return Err(wrt_error::Error::runtime_trap("array.get_u: expected i32 index")),
                        };
                        if let Some(Value::ArrayRef(Some(a))) = operand_stack.pop() {
                            if let Ok(elem) = a.get(index) {
                                operand_stack.push(elem.clone());
                            } else {
                                return Err(wrt_error::Error::runtime_trap("array.get_u: index out of bounds"));
                            }
                        } else {
                            return Err(wrt_error::Error::runtime_trap("array.get_u: null reference"));
                        }
                    }

                    Instruction::ArraySet(type_idx) => {
                        // array.set: [arrayref i32 value] -> []
                        #[cfg(feature = "tracing")]
                        trace!("ArraySet: type_idx={}", type_idx);
                        let value = operand_stack.pop().ok_or_else(||
                            wrt_error::Error::runtime_trap("array.set: expected value"))?;
                        let index = match operand_stack.pop() {
                            Some(Value::I32(n)) => n as usize,
                            _ => return Err(wrt_error::Error::runtime_trap("array.set: expected i32 index")),
                        };
                        if let Some(Value::ArrayRef(Some(mut a))) = operand_stack.pop() {
                            a.set(index, value).map_err(|_|
                                wrt_error::Error::runtime_trap("array.set: index out of bounds"))?;
                        } else {
                            return Err(wrt_error::Error::runtime_trap("array.set: null reference"));
                        }
                    }

                    Instruction::ArrayLen => {
                        // array.len: [arrayref] -> [i32]
                        #[cfg(feature = "tracing")]
                        trace!("ArrayLen");
                        if let Some(Value::ArrayRef(Some(a))) = operand_stack.pop() {
                            operand_stack.push(Value::I32(a.len() as i32));
                        } else {
                            return Err(wrt_error::Error::runtime_trap("array.len: null reference"));
                        }
                    }

                    Instruction::RefI31 => {
                        // ref.i31: [i32] -> [i31ref]
                        #[cfg(feature = "tracing")]
                        trace!("RefI31");
                        if let Some(Value::I32(n)) = operand_stack.pop() {
                            // Truncate to 31 bits (sign-extend from 31 bits)
                            let i31_val = (n << 1) >> 1;
                            operand_stack.push(Value::I31Ref(Some(i31_val)));
                        } else {
                            return Err(wrt_error::Error::runtime_trap("ref.i31: expected i32"));
                        }
                    }

                    Instruction::I31GetS => {
                        // i31.get_s: [i31ref] -> [i32] (sign-extended)
                        #[cfg(feature = "tracing")]
                        trace!("I31GetS");
                        match operand_stack.pop() {
                            Some(Value::I31Ref(Some(n))) => {
                                operand_stack.push(Value::I32(n));
                            }
                            Some(Value::I31Ref(None)) => {
                                return Err(wrt_error::Error::runtime_trap("i31.get_s: null reference"));
                            }
                            _ => {
                                return Err(wrt_error::Error::runtime_trap("i31.get_s: expected i31ref"));
                            }
                        }
                    }

                    Instruction::I31GetU => {
                        // i31.get_u: [i31ref] -> [i32] (zero-extended)
                        #[cfg(feature = "tracing")]
                        trace!("I31GetU");
                        match operand_stack.pop() {
                            Some(Value::I31Ref(Some(n))) => {
                                // Zero-extend: mask to 31 bits
                                operand_stack.push(Value::I32(n & 0x7FFFFFFF));
                            }
                            Some(Value::I31Ref(None)) => {
                                return Err(wrt_error::Error::runtime_trap("i31.get_u: null reference"));
                            }
                            _ => {
                                return Err(wrt_error::Error::runtime_trap("i31.get_u: expected i31ref"));
                            }
                        }
                    }

                    // GC instructions that need more context (type info, etc.)
                    // These return stubs for now - full implementation requires type section data
                    Instruction::ArrayNewData(_, _) |
                    Instruction::ArrayNewElem(_, _) |
                    Instruction::ArrayFill(_) |
                    Instruction::ArrayCopy(_, _) |
                    Instruction::ArrayInitData(_, _) |
                    Instruction::ArrayInitElem(_, _) |
                    Instruction::RefTest(_) |
                    Instruction::RefTestNull(_) |
                    Instruction::RefCast(_) |
                    Instruction::RefCastNull(_) |
                    Instruction::BrOnCast { .. } |
                    Instruction::BrOnCastFail { .. } |
                    Instruction::AnyConvertExtern |
                    Instruction::ExternConvertAny => {
                        #[cfg(feature = "tracing")]
                        trace!("GC instruction (stub): {:?}", instruction);
                        // These instructions require more complex type system integration
                        // For now, return an error indicating incomplete implementation
                        return Err(wrt_error::Error::runtime_error(
                            "GC instruction not yet fully implemented",
                        ));
                    }

                    _ => {
                        // CLAUDE.md: FAIL LOUD AND EARLY - return error instead of silently skipping
                        #[cfg(feature = "tracing")]
                        error!("Unimplemented instruction at pc={}: {:?}", pc, instruction);
                        return Err(wrt_error::Error::runtime_error(
                            "Unimplemented instruction encountered",
                        ));
                    }
                }

                // Increment program counter for next iteration
                pc += 1;
            }

            // Return values from operand stack matching function signature
            #[cfg(feature = "tracing")]

            trace!("Function complete. Operand stack has {} values", operand_stack.len());
            #[cfg(feature = "tracing")]

            trace!("STATS: Executed {} instructions total", instruction_count);
            #[cfg(feature = "tracing")]

            trace!("Function type expects {} results", func_type.results.len());

            let mut results = Vec::new();
            for (i, result_type) in func_type.results.iter().enumerate() {
                if let Some(value) = operand_stack.pop() {
                    #[cfg(feature = "tracing")]

                    trace!("Result {}: {:?}", i, value);
                    results.insert(0, value);
                } else {
                    #[cfg(feature = "tracing")]

                    trace!("Result {}: missing, using default", i);
                    results.insert(0, Value::I32(0));
                }
            }

            #[cfg(feature = "tracing")]
            trace!("Returning {} results", results.len());

            // Restore debugger back to self
            #[cfg(all(feature = "std", feature = "debugger"))]
            {
                self.debugger = debugger_opt;
            }

            // Return completed execution - call depth is handled by the trampoline wrapper
            Ok(ExecutionOutcome::Complete(results))
        }

        #[cfg(not(feature = "std"))]
        {
            // Fallback for no_std - return default values
            let mut results = {
                use wrt_foundation::{
                    budget_aware_provider::CrateId,
                    safe_managed_alloc,
                };
                use crate::bounded_runtime_infra::RUNTIME_MEMORY_SIZE;
                let provider = safe_managed_alloc!(RUNTIME_MEMORY_SIZE, CrateId::Runtime)?;
                BoundedVec::new(provider)?
            };
            for result_type in &func_type.results {
                let default_value = match result_type {
                    wrt_foundation::ValueType::I32 => Value::I32(0),
                    wrt_foundation::ValueType::I64 => Value::I64(0),
                    wrt_foundation::ValueType::F32 => Value::F32(FloatBits32(0)),
                    wrt_foundation::ValueType::F64 => Value::F64(FloatBits64(0)),
                    _ => Value::I32(0),
                };
                results.push(default_value)?;
            }
            // Return completed execution - call depth is handled by the trampoline wrapper
            Ok(ExecutionOutcome::Complete(results))
        }
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    pub fn execute(
        &self,
        instance_id: usize,
        func_idx: usize,
        args: Vec<Value>,
    ) -> Result<Vec<Value>> {
        #[cfg(feature = "std")]
        #[cfg(feature = "tracing")]

        trace!("DEBUG StacklessEngine::execute: instance_id={}, func_idx={}", instance_id, func_idx);

        let instance = self
            .instances
            .get(&instance_id)?
            .ok_or_else(|| wrt_error::Error::runtime_execution_error("Instance not found"))?;

        // For now, implement a basic execution that validates the function exists
        // and returns appropriate results
        let module = instance.module();

        #[cfg(feature = "std")]
        #[cfg(feature = "tracing")]

        debug!("Got module, functions.len()={}", module.functions.len());

        // Validate function index
        if func_idx >= module.functions.len() {
            return Err(wrt_error::Error::runtime_function_not_found(
                "Function index out of bounds",
            ));
        }

        let func = module
            .functions
            .get(func_idx)
            .map_err(|_| wrt_error::Error::runtime_error("Failed to get function"))?;

        #[cfg(feature = "std")]
        #[cfg(feature = "tracing")]

        debug!("Retrieved func, body.instructions.len()={}", func.body.instructions.len());

        #[cfg(feature = "std")]
        #[cfg(feature = "tracing")]

        trace!("DEBUG execute: func.type_idx={}, module.types.len()={}", func.type_idx, module.types.len());

        // In std mode, types is Vec so get() returns Option<&T>
        #[cfg(feature = "std")]
        let func_type = module
            .types
            .get(func.type_idx as usize)
            .ok_or_else(|| wrt_error::Error::runtime_error("Failed to get function type"))?;

        // In no_std mode, types is BoundedVec so get() returns Result<T>
        #[cfg(not(feature = "std"))]
        let func_type = &module
            .types
            .get(func.type_idx as usize)
            .map_err(|_| wrt_error::Error::runtime_error("Failed to get function type"))?;

        // Return appropriate default values based on function signature
        let mut results = {
            use wrt_foundation::{
                budget_aware_provider::CrateId,
                safe_managed_alloc,
            };

            let provider = safe_managed_alloc!(4096, CrateId::Runtime)?;
            BoundedVec::new(provider)
                .map_err(|_| wrt_error::Error::runtime_error("Failed to create results vector"))?
        };
        for result_type in &func_type.results {
            let default_value = match result_type {
                wrt_foundation::ValueType::I32 => Value::I32(0),
                wrt_foundation::ValueType::I64 => Value::I64(0),
                wrt_foundation::ValueType::F32 => Value::F32(FloatBits32(0.0f32.to_bits())),
                wrt_foundation::ValueType::F64 => Value::F64(FloatBits64(0.0f64.to_bits())),
                // Add other types as needed
                _ => Value::I32(0), // Default fallback
            };
            results
                .push(default_value)
                .map_err(|_| wrt_error::Error::runtime_error("Failed to push result value"))?;
        }

        Ok(results)
    }

    /// Get the remaining fuel for execution
    pub fn remaining_fuel(&self) -> Option<u64> {
        Some(self.fuel.load(Ordering::Relaxed))
    }

    /// Get the current instruction pointer
    pub fn get_instruction_pointer(&self) -> Result<u32> {
        Ok(self.instruction_pointer.load(Ordering::Relaxed) as u32)
    }

    /// Execute a single step of function execution with instruction limit
    pub fn execute_function_step(
        &mut self,
        instance: &ModuleInstance,
        func_idx: usize,
        params: &[Value],
        max_instructions: u32,
    ) -> Result<crate::stackless::ExecutionResult> {
        use wrt_foundation::{
            budget_aware_provider::CrateId,
            safe_managed_alloc,
        };

        // Validate function exists
        let module = instance.module();
        if func_idx >= module.functions.len() {
            return Err(wrt_error::Error::runtime_function_not_found(
                "Function index out of bounds",
            ));
        }

        // Get function type
        let func = module
            .functions
            .get(func_idx)
            .ok_or_else(|| wrt_error::Error::runtime_function_not_found("Failed to get function"))?;
        // In std mode, types is Vec so get() returns Option<&T>
        #[cfg(feature = "std")]
        let func_type = module
            .types
            .get(func.type_idx as usize)
            .ok_or_else(|| wrt_error::Error::runtime_error("Failed to get function type"))?;

        // In no_std mode, types is BoundedVec so get() returns Result<T>
        #[cfg(not(feature = "std"))]
        let func_type = &module
            .types
            .get(func.type_idx as usize)
            .map_err(|_| wrt_error::Error::runtime_error("Failed to get function type"))?;

        // Simulate step execution - in real implementation would execute instructions
        // For now, return completed with default values
        let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
        let mut results = wrt_foundation::bounded::BoundedVec::new(provider)
            .map_err(|_| wrt_error::Error::runtime_error("Failed to create results vector"))?;

        for result_type in &func_type.results {
            let default_value = match result_type {
                wrt_foundation::ValueType::I32 => Value::I32(0),
                wrt_foundation::ValueType::I64 => Value::I64(0),
                wrt_foundation::ValueType::F32 => Value::F32(FloatBits32(0.0f32.to_bits())),
                wrt_foundation::ValueType::F64 => Value::F64(FloatBits64(0.0f64.to_bits())),
                _ => Value::I32(0),
            };
            results
                .push(default_value)
                .map_err(|_| wrt_error::Error::runtime_error("Failed to push result value"))?;
        }

        // Update instruction pointer
        self.instruction_pointer
            .fetch_add(max_instructions as u64, Ordering::Relaxed);

        // Consume some fuel
        let fuel_to_consume = max_instructions.min(100) as u64;
        let current_fuel = self.fuel.load(Ordering::Relaxed);
        if current_fuel < fuel_to_consume {
            self.fuel.store(0, Ordering::Relaxed);
            return Ok(crate::stackless::ExecutionResult::FuelExhausted);
        }
        self.fuel
            .fetch_sub(fuel_to_consume, Ordering::Relaxed);

        Ok(crate::stackless::ExecutionResult::Completed(results))
    }

    /// Restore engine state from a saved state
    pub fn restore_state(&mut self, state: crate::stackless::EngineState) -> Result<()> {
        self.instruction_pointer
            .store(state.instruction_pointer as u64, Ordering::Relaxed);

        // In a real implementation, would restore operand stack, locals, and call stack
        // For now, just update the instruction pointer
        Ok(())
    }

    /// Continue execution from current state
    pub fn continue_execution(
        &mut self,
        max_instructions: u32,
    ) -> Result<crate::stackless::ExecutionResult> {
        use wrt_foundation::{
            budget_aware_provider::CrateId,
            safe_managed_alloc,
        };

        // Simulate continued execution
        // In real implementation, would resume from saved state

        // Update instruction pointer
        self.instruction_pointer
            .fetch_add(max_instructions as u64, Ordering::Relaxed);

        // Consume some fuel
        let fuel_to_consume = max_instructions.min(100) as u64;
        let current_fuel = self.fuel.load(Ordering::Relaxed);
        if current_fuel < fuel_to_consume {
            self.fuel.store(0, Ordering::Relaxed);
            return Ok(crate::stackless::ExecutionResult::FuelExhausted);
        }
        self.fuel
            .fetch_sub(fuel_to_consume, Ordering::Relaxed);

        // For now, return completed with empty results
        let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
        let results = wrt_foundation::bounded::BoundedVec::new(provider)
            .map_err(|_| wrt_error::Error::runtime_error("Failed to create results vector"))?;

        Ok(crate::stackless::ExecutionResult::Completed(results))
    }

    /// Count total number of imports across all modules
    fn count_total_imports(&self, module: &crate::module::Module) -> usize {
        // For now, we'll count based on the fact that imported functions
        // are added as placeholder functions at the beginning of the functions array
        // A proper implementation would iterate through module.imports

        // Count functions that are imports (those with empty body)
        let mut import_count = 0;
        for func in &module.functions {
            if func.body.is_empty() && func.locals.is_empty() {
                import_count += 1;
            } else {
                // Once we hit a non-import function, we're done
                break;
            }
        }

        #[cfg(feature = "tracing")]


        trace!("Total imports counted: {}", import_count);
        import_count
    }

    /// Find import by function index using the ordered import list
    ///
    /// IMPORTANT: func_idx is the FUNCTION import index (0 = first imported function),
    /// not the overall import index. import_order contains ALL imports (functions,
    /// globals, memories, tables, tags), so we must iterate through import_types
    /// to find the Nth function import.
    fn find_import_by_index(&self, module: &crate::module::Module, func_idx: usize) -> Result<(String, String)> {
        use crate::module::RuntimeImportDesc;

        #[cfg(feature = "tracing")]
        let _span = wrt_foundation::tracing::ImportTrace::lookup("", "").entered();

        #[cfg(feature = "tracing")]
        debug!("Looking for function import at index {} (import_order.len()={}, import_types.len()={})",
               func_idx, module.import_order.len(), module.import_types.len());

        // Iterate through ALL imports, counting only function imports until we find the one we want
        #[cfg(feature = "std")]
        {
            let mut func_import_count = 0usize;
            for (i, (module_name, field_name)) in module.import_order.iter().enumerate() {
                // Check if this import is a function import
                if let Some(RuntimeImportDesc::Function(_)) = module.import_types.get(i) {
                    if func_import_count == func_idx {
                        #[cfg(feature = "tracing")]
                        trace!("Found function import {} at overall index {}: {}::{}",
                               func_idx, i, module_name, field_name);
                        return Ok((module_name.clone(), field_name.clone()));
                    }
                    func_import_count += 1;
                }
            }
        }

        #[cfg(not(feature = "std"))]
        {
            let mut func_import_count = 0usize;
            for i in 0..module.import_order.len() {
                if let Ok(Some((module_name, field_name))) = module.import_order.get(i) {
                    // Check if this import is a function import
                    if let Some(RuntimeImportDesc::Function(_)) = module.import_types.get(i) {
                        if func_import_count == func_idx {
                            #[cfg(feature = "tracing")]
                            trace!("Found function import {} at overall index {}: {}::{}",
                                   func_idx, i,
                                   module_name.as_str().unwrap_or("<error>"),
                                   field_name.as_str().unwrap_or("<error>"));
                            return Ok((
                                module_name.as_str().map(|s| s.to_string()).unwrap_or_default(),
                                field_name.as_str().map(|s| s.to_string()).unwrap_or_default()
                            ));
                        }
                        func_import_count += 1;
                    }
                }
            }
        }

        #[cfg(feature = "tracing")]
        trace!("Could not find function import at index {} (import_order.len()={})", func_idx, module.import_order.len());
        Err(wrt_error::Error::runtime_error("Import not found"))
    }

    /// Collect function arguments from operand stack based on function signature
    fn collect_function_args(module: &crate::module::Module, func_idx: usize, operand_stack: &mut Vec<Value>) -> Vec<Value> {
        // Get the function's type signature to determine param count
        let param_count = if let Some(func) = module.functions.get(func_idx) {
            if let Some(func_type) = module.types.get(func.type_idx as usize) {
                func_type.params.len()
            } else {
                0
            }
        } else {
            0
        };

        // Pop arguments from stack in reverse order
        let mut args = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            if let Some(val) = operand_stack.pop() {
                args.push(val);
            }
        }
        args.reverse(); // Reverse to get correct order
        args
    }

    /// Get the parameter count for an imported function
    fn get_import_param_count(module: &crate::module::Module, import_idx: usize) -> usize {
        use wrt_foundation::types::ImportDesc;

        // Look up the import's type from import_types
        if let Some(import_desc) = module.import_types.get(import_idx) {
            match import_desc {
                ImportDesc::Function(type_idx) => {
                    // Get the function type and return param count
                    if let Some(func_type) = module.types.get(*type_idx as usize) {
                        return func_type.params.len();
                    }
                }
                _ => {} // Not a function import
            }
        }
        0 // Default to 0 if we can't determine
    }

    /// Collect import function arguments from operand stack
    fn collect_import_args(module: &crate::module::Module, import_idx: usize, operand_stack: &mut Vec<Value>) -> Vec<Value> {
        let param_count = Self::get_import_param_count(module, import_idx);

        // Pop arguments from stack in reverse order
        let mut args = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            if let Some(val) = operand_stack.pop() {
                args.push(val);
            }
        }
        args.reverse(); // Reverse to get correct order
        args
    }

    /// Find export function index by name
    fn find_export_index(&self, module: &crate::module::Module, name: &str) -> Result<usize> {
        #[cfg(feature = "std")]
        {
            for (export_name, export) in module.exports.iter() {
                // BoundedString::as_str() returns Result<&str, BoundedError>
                if let Ok(export_str) = export_name.as_str() {
                    if export_str == name {
                        if let crate::module::ExportKind::Function = export.kind {
                            return Ok(export.index as usize);
                        }
                    }
                }
            }
        }

        #[cfg(not(feature = "std"))]
        {
            for (export_name, export) in module.exports.iter() {
                if export_name.as_str() == name {
                    if let crate::module::ExportKind::Function = export.kind {
                        return Ok(export.index as usize);
                    }
                }
            }
        }

        Err(wrt_error::Error::runtime_error("Export function not found"))
    }

    /// Find an import's parameter count by module and field name
    #[cfg(feature = "std")]
    fn get_import_param_count_by_name(
        module: &crate::module::Module,
        module_name: &str,
        field_name: &str,
    ) -> usize {
        use wrt_foundation::types::ImportDesc;

        // Strip version from module name for matching
        let base_module = strip_wasi_version(module_name);

        // Use import_order (Vec<(String, String)>) to find the import index,
        // then look up its type in import_types
        for (idx, (imp_module, imp_name)) in module.import_order.iter().enumerate() {
            let base_imp_module = strip_wasi_version(imp_module);
            if base_imp_module == base_module && imp_name == field_name {
                // Found matching import - get its type from import_types
                if let Some(import_desc) = module.import_types.get(idx) {
                    match import_desc {
                        ImportDesc::Function(type_idx) => {
                            if let Some(func_type) = module.types.get(*type_idx as usize) {
                                return func_type.params.len();
                            }
                        }
                        _ => {} // Not a function import
                    }
                }
            }
        }
        0 // Default to 0 if not found
    }

    /// Collect import arguments from stack by module/field name
    #[cfg(feature = "std")]
    fn collect_import_args_by_name(
        module: &crate::module::Module,
        module_name: &str,
        field_name: &str,
        operand_stack: &mut Vec<Value>,
    ) -> Vec<Value> {
        let param_count = Self::get_import_param_count_by_name(module, module_name, field_name);

        // Pop arguments from stack in reverse order
        let mut args = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            if let Some(val) = operand_stack.pop() {
                args.push(val);
            }
        }
        args.reverse(); // Reverse to get correct order
        args
    }

    /// Call cabi_realloc to allocate memory in WASM instance
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn call_cabi_realloc(&mut self, instance_id: usize, func_idx: usize,
                         old_ptr: u32, old_size: u32, align: u32, new_size: u32) -> Result<u32> {
        #[cfg(feature = "tracing")]
        trace!(
            old_ptr = old_ptr,
            old_size = old_size,
            align = align,
            new_size = new_size,
            "[CABI_REALLOC] Calling"
        );
        let args = vec![
            Value::I32(old_ptr as i32),
            Value::I32(old_size as i32),
            Value::I32(align as i32),
            Value::I32(new_size as i32),
        ];
        #[cfg(feature = "tracing")]
        trace!(args = ?args, "[CABI_REALLOC] Arguments prepared");

        // Use execute_leaf_function instead of execute() to avoid nested trampolines.
        // cabi_realloc is guaranteed by canonical ABI to be a leaf function (no calls).
        let results = self.execute_leaf_function(instance_id, func_idx, args)?;

        if let Some(Value::I32(ptr)) = results.first() {
            Ok(*ptr as u32)
        } else {
            Err(wrt_error::Error::runtime_error("cabi_realloc returned invalid value"))
        }
    }

    /// Allocate memory for WASI arguments using cabi_realloc
    ///
    /// This properly allocates memory owned by the component so that argument
    /// strings don't get overwritten by the component's heap/stack operations.
    ///
    /// IMPORTANT: Each string is allocated SEPARATELY via cabi_realloc to ensure
    /// proper allocator metadata. This allows the component to free strings
    /// individually without causing allocator corruption.
    ///
    /// Returns (list_ptr, string_ptrs) where:
    /// - list_ptr: pointer to the (ptr, len) array for list elements
    /// - string_ptrs: vector of (ptr, len) for each string allocation
    #[cfg(feature = "wasi")]
    fn allocate_wasi_args_memory(
        &mut self,
        instance_id: usize,
        args: &[String],
    ) -> Result<Option<(u32, Vec<(u32, u32)>)>> {
        // Find cabi_realloc export
        let instance = self.instances.get(&instance_id)
            .ok_or_else(|| wrt_error::Error::runtime_error("Instance not found"))?
            .clone();
        let module = instance.module();

        let cabi_realloc_idx = match self.find_export_index(&module, "cabi_realloc") {
            Ok(idx) => idx,
            Err(_) => {
                #[cfg(feature = "tracing")]
                trace!("[WASI-ALLOC] cabi_realloc not found, falling back to stack-relative allocation");
                return Ok(None);
            }
        };

        if args.is_empty() {
            return Ok(None);
        }

        // Calculate list array size: 8 bytes per arg (ptr + len)
        let list_size = args.len() * 8;

        #[cfg(feature = "tracing")]
        trace!(
            args_len = args.len(),
            list_size = list_size,
            "[WASI-ALLOC] Allocating memory for args with SEPARATE string allocations"
        );

        // Allocate the list array (holds ptr+len pairs)
        let list_ptr = self.call_cabi_realloc(
            instance_id,
            cabi_realloc_idx,
            0,  // old_ptr (NULL for new allocation)
            0,  // old_size
            8,  // align (8-byte alignment for pointers)
            list_size as u32,
        )?;

        #[cfg(feature = "tracing")]
        trace!(list_ptr = format_args!("0x{:x}", list_ptr), "[WASI-ALLOC] list array allocated");

        // Allocate EACH string separately to ensure proper allocator metadata
        // This is critical: dlmalloc needs metadata before each allocation
        let mut string_ptrs: Vec<(u32, u32)> = Vec::with_capacity(args.len());

        for (i, arg) in args.iter().enumerate() {
            let string_len = arg.len() as u32;

            // Allocate memory for this string (alignment 1 for byte data)
            let string_ptr = self.call_cabi_realloc(
                instance_id,
                cabi_realloc_idx,
                0,  // old_ptr (NULL for new allocation)
                0,  // old_size
                1,  // align (1-byte alignment for strings)
                string_len,
            )?;

            #[cfg(feature = "tracing")]
            trace!(
                index = i,
                string_ptr = format_args!("0x{:x}", string_ptr),
                string_len = string_len,
                arg = %arg,
                "[WASI-ALLOC] string allocated separately"
            );

            string_ptrs.push((string_ptr, string_len));
        }

        Ok(Some((list_ptr, string_ptrs)))
    }

    /// Pre-allocate memory for WASI arguments before starting execution
    ///
    /// This should be called before `execute` to allocate memory via cabi_realloc
    /// for WASI argument strings. The allocated pointers will be stored in the
    /// WASI dispatcher and used when get-arguments is called.
    ///
    /// # Arguments
    /// * `instance_id` - The instance ID to allocate memory in
    ///
    /// # Returns
    /// Ok(()) if successful, Err if allocation fails
    #[cfg(feature = "wasi")]
    pub fn pre_allocate_wasi_args(&mut self, instance_id: usize) -> Result<()> {
        // Get args from global state (set by wrtd before execution)
        // This is more reliable than using internal dispatcher since it
        // ensures we use the same args that will be returned by get-arguments
        let args: Vec<String> = wrt_wasi::get_global_wasi_args();

        if args.is_empty() {
            #[cfg(feature = "tracing")]
            trace!("[WASI-PREALLOC] No args to pre-allocate");
            return Ok(());
        }

        #[cfg(feature = "tracing")]
        trace!(
            args_len = args.len(),
            args = ?args,
            "[WASI-PREALLOC] Pre-allocating memory for args"
        );

        // Allocate memory - each string is allocated SEPARATELY for proper allocator metadata
        match self.allocate_wasi_args_memory(instance_id, &args)? {
            Some((list_ptr, string_ptrs)) => {
                #[cfg(feature = "tracing")]
                trace!(
                    list_ptr = format_args!("0x{:x}", list_ptr),
                    num_strings = string_ptrs.len(),
                    "[WASI-PREALLOC] Allocated successfully with SEPARATE string allocations"
                );

                // Store in internal dispatcher (for backwards compatibility)
                if let Some(ref mut dispatcher) = self.wasi_dispatcher {
                    dispatcher.set_args_alloc(list_ptr, string_ptrs.clone());
                }

                // CRITICAL: Also set on host_handler (the actual dispatch target)
                // This is the dispatcher that receives get-arguments calls
                #[cfg(feature = "std")]
                if let Some(ref mut handler) = self.host_handler {
                    handler.set_args_allocation(list_ptr, string_ptrs);
                }

                Ok(())
            }
            None => {
                #[cfg(feature = "tracing")]
                trace!("[WASI-PREALLOC] No allocation needed or cabi_realloc not available");
                Ok(())
            }
        }
    }

    /// Write data to WASM instance memory
    fn write_to_instance(&self, instance_id: usize, addr: u32, data: &[u8]) -> Result<()> {
        let instance = self.instances.get(&instance_id)
            .ok_or_else(|| wrt_error::Error::runtime_error("Instance not found"))?;

        #[cfg(feature = "tracing")]


        debug!("WASI init: write_to_instance: module declares {} memories",                   if instance.module().memories.is_empty() { 0 } else { instance.module().memories.len() });

        // Try to get memory - this will fail if instance doesn't have runtime memory initialized
        let memory = match instance.memory(0) {
            Ok(mem) => mem,
            Err(e) => {
                #[cfg(feature = "tracing")]

                debug!("WASI init: write_to_instance: failed to get memory: {:?}", e);
                return Err(e);
            }
        };

        let pages = memory.0.size();  // This is the size() method that returns pages
        let bytes = pages as usize * 65536;  // Convert pages to bytes
        #[cfg(feature = "tracing")]

        debug!("WASI init: write_to_instance: memory {} pages = {} bytes", pages, bytes);

        // Verify the write won't exceed memory bounds
        if (addr as usize + data.len()) > bytes {
            #[cfg(feature = "tracing")]

            debug!("WASI init: write_to_instance: write at {:#x} + {} bytes would exceed {} byte memory",                      addr, data.len(), bytes);
            return Err(wrt_error::Error::runtime_execution_error("Write would exceed memory bounds"));
        }

        memory.0.write_shared(addr, data)?;
        Ok(())
    }

    /// Initialize WASI stub memory for an instance
    ///
    /// For now, we use a simple fixed address (0x100) for stub data since memory cloning
    /// has issues preserving size. This is safe because:
    /// - Address 0x100 (256 bytes) is well within even small WASM memories
    /// - The data is just zeros which won't interfere with normal operation
    /// - WASM code that reads these pointers will get valid empty lists/None values
    fn initialize_wasi_stubs(&mut self, instance_id: usize, module: &crate::module::Module) -> Result<()> {
        #[cfg(feature = "tracing")]

        debug!("WASI init: Initializing WASI stubs for instance {}", instance_id);

        // Use a low fixed address that's guaranteed to be valid in most WASM memories
        // We need 16 bytes total: 8 for empty list + 1 for option None + 7 padding
        let base_ptr = 0x100u32; // 256 bytes into memory

        // Try to write stub data - if this fails, memory isn't ready yet (which is normal for many instances)
        match self.write_to_instance(instance_id, base_ptr, &[0u8; 16]) {
            Ok(_) => {
                #[cfg(feature = "tracing")]

                info!("WASI init: ✓ Wrote 16 bytes of stub data at ptr={:#x}", base_ptr);

                // Cache the pointers
                let stub_mem = WasiStubMemory {
                    empty_list: base_ptr,      // Points to 8 bytes of zeros = (ptr=0, len=0)
                    option_none: base_ptr + 8, // Points to 1 byte zero = None
                    empty_env: base_ptr,       // Reuse empty_list
                    stdout_handle: 1,
                    stderr_handle: 2,
                };

                self.wasi_stubs.insert(instance_id, stub_mem);
                #[cfg(feature = "tracing")]

                info!("WASI init: ✓ WASI stubs initialized with memory write");
                Ok(())
            }
            Err(_e) => {
                #[cfg(feature = "tracing")]

                debug!("WASI init: Instance has no accessible memory (normal for adapter modules)");
                #[cfg(feature = "tracing")]

                debug!("WASI init: Using fallback pointers (stub WASI functions will return empty values)");

                // Even if we can't write, we can still return valid pointers
                // The WASM memory likely has zeros at these addresses anyway
                let stub_mem = WasiStubMemory {
                    empty_list: base_ptr,
                    option_none: base_ptr + 8,
                    empty_env: base_ptr,
                    stdout_handle: 1,
                    stderr_handle: 2,
                };

                self.wasi_stubs.insert(instance_id, stub_mem);
                Ok(()) // Don't fail - just use the pointers anyway
            }
        }
    }

    /// Call a WASI host function
    fn call_wasi_function(
        &mut self,
        module_name: &str,
        field_name: &str,
        stack: &mut Vec<Value>,
        module: &crate::module::Module,
        instance_id: usize,
    ) -> Result<Option<Value>> {
        #[cfg(feature = "std")]
        use std::io::Write;

        #[cfg(feature = "tracing")]
        debug!("WASI: Calling {}::{}", module_name, field_name);

        // NOTE: We skip the host_registry path for WASI functions because it doesn't
        // properly marshal arguments from the stack. Instead, we fall through to the
        // wasip2_host dispatch (for WASI Preview 2) or the stub implementations which
        // properly handle the stack arguments.
        //
        // The host_registry is kept for non-WASI host functions that don't need stack args.
        #[cfg(feature = "std")]
        if let Some(ref registry) = self.host_registry {
            // Only use host_registry for non-WASI functions
            // WASI functions need proper stack argument marshalling done below
            if !module_name.starts_with("wasi:") {
                #[cfg(feature = "tracing")]
                debug!("WASI: Checking host registry for {}::{}", module_name, field_name);

                if registry.has_host_function(module_name, field_name) {
                    #[cfg(feature = "tracing")]
                    debug!("WASI: Found {} in host registry, calling implementation", field_name);

                    let args: Vec<wrt_foundation::Value> = vec![];
                    let mut dummy_engine: i32 = 0;
                    match registry.call_host_function(&mut dummy_engine, module_name, field_name, args) {
                        Ok(result) => {
                            #[cfg(feature = "tracing")]
                            debug!("WASI: Host function {} returned successfully", field_name);
                            if let Some(val) = result.first() {
                                return Ok(Some(val.clone()));
                            }
                            return Ok(None);
                        }
                        Err(e) => {
                            #[cfg(feature = "tracing")]
                            debug!("WASI: Host function {} failed: {:?}", field_name, e);
                            return Err(e);
                        }
                    }
                }
            }
        }

        // Use host_handler for WASI dispatch (when configured)
        // This is the proper architecture - host_handler routes to WasiDispatcher
        #[cfg(feature = "std")]
        {
            // ON-DEMAND ALLOCATION for get-arguments
            // This MUST happen AFTER _start has initialized the component's allocator.
            // Pre-allocating before _start causes memory collisions where the allocator
            // reuses our memory for other purposes.
            #[cfg(feature = "wasi")]
            let args_alloc = if module_name.contains("wasi:cli/environment") && field_name == "get-arguments" {
                let wasi_args = wrt_wasi::get_global_wasi_args();
                if !wasi_args.is_empty() {
                    #[cfg(feature = "tracing")]
                    trace!(
                        args = ?wasi_args,
                        "[ON-DEMAND-ALLOC] Allocating memory for get-arguments"
                    );
                    match self.allocate_wasi_args_memory(instance_id, &wasi_args) {
                        Ok(alloc) => alloc,
                        Err(e) => {
                            #[cfg(feature = "tracing")]
                            warn!(error = %e, "[ON-DEMAND-ALLOC] Failed to allocate args memory");
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };

            #[cfg(not(feature = "wasi"))]
            let args_alloc: Option<(u32, Vec<(u32, u32)>)> = None;

            if let Some(ref mut handler) = self.host_handler {
                // Set the allocation if we have one (for get-arguments)
                // Each string is allocated SEPARATELY to ensure proper allocator metadata
                if let Some((list_ptr, string_ptrs)) = args_alloc {
                    #[cfg(feature = "tracing")]
                    trace!(
                        list_ptr = format_args!("0x{:x}", list_ptr),
                        num_strings = string_ptrs.len(),
                        "[ON-DEMAND-ALLOC] Setting args allocation on handler with SEPARATE strings"
                    );
                    handler.set_args_allocation(list_ptr, string_ptrs);
                }

                #[cfg(feature = "tracing")]
                debug!(
                    module_name = %module_name,
                    field_name = %field_name,
                    "[HOST_HANDLER] Dispatching via HostImportHandler"
                );

                // Get instance and memory
                let instance = self.instances.get(&instance_id)
                    .ok_or_else(|| wrt_error::Error::runtime_error("Instance not found for host handler call"))?
                    .clone();

                // Get memory as MemoryAccessor
                let mem_wrapper = instance.memory(0).ok();
                let memory: Option<&dyn wrt_foundation::MemoryAccessor> = mem_wrapper.as_ref()
                    .map(|m| m.0.as_ref() as &dyn wrt_foundation::MemoryAccessor);

                // Collect args from stack based on function signature
                let args = Self::collect_import_args_by_name(&module, module_name, field_name, stack);

                #[cfg(feature = "tracing")]
                trace!(
                    args_count = args.len(),
                    has_memory = memory.is_some(),
                    "[HOST_HANDLER] Calling handler"
                );

                // Call handler - errors propagate up (no fallback per CLAUDE.md)
                let results = handler.call_import(module_name, field_name, &args, memory)?;

                #[cfg(feature = "tracing")]
                trace!(
                    results_count = results.len(),
                    "[HOST_HANDLER] Handler returned successfully"
                );

                // Return first result if any
                return Ok(results.into_iter().next());
            }
        }

        // NO INLINE WASI DISPATCH - per CLAUDE.md NO FALLBACK LOGIC rule
        // All WASI calls MUST go through host_handler (set via set_host_handler())
        // The host_handler routes to WasiDispatcher which handles all WASI functions
        //
        // If we reach here, it means:
        // 1. host_handler was not configured (wrtd should always configure it)
        // 2. The caller is using the engine without proper WASI setup
        //
        // This is NOT a fallback - it's a hard error per CLAUDE.md:
        // "FAIL LOUD AND EARLY: If data is missing or incorrect, return an error immediately"
        #[cfg(feature = "tracing")]
        error!(
            module_name = %module_name,
            field_name = %field_name,
            "[WASI] No host_handler configured - cannot dispatch WASI function"
        );

        Err(wrt_error::Error::runtime_error(
            "WASI function called but no host_handler configured. \
             Use engine.set_host_handler() with a WasiDispatcher to enable WASI support."
        ))
    }
}

// REMOVED: ~950 lines of inline WASI dispatch code
// All WASI dispatch is now handled by WasiDispatcher via HostImportHandler trait
// See wrt-wasi/src/dispatcher.rs for the implementation

impl Default for StacklessEngine {
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn default() -> Self {
        Self::new()
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    fn default() -> Self {
        Self::new().expect("Failed to create default StacklessEngine in no_std mode")
    }
}

// Additional types that might be needed - using simple type aliases to avoid
// conflicts
/// Type alias for callback registry (placeholder implementation).
pub type StacklessCallbackRegistry = ();
/// Type alias for execution stack (placeholder implementation).
pub type StacklessStack = ();
