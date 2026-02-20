//! WASI Preview2 (wasip2) Host Implementation
//!
//! This module provides host implementations for WASI preview2 interfaces
//! that are used by WebAssembly components through the Component Model.
//!
//! # Two Interfaces
//!
//! This module provides two dispatch interfaces:
//!
//! 1. **`dispatch`** - Low-level interface using core WASM values (`Value`)
//!    - Arguments come directly from WASM operand stack
//!    - Complex types passed as ptr+len (requires memory access)
//!    - Used by direct engine execution path
//!
//! 2. **`dispatch_component`** - Component Model interface using `ComponentValue`
//!    - Arguments are proper component values (strings, lists, etc.)
//!    - Complex types are first-class values, not memory pointers
//!    - Used when canonical ABI lift/lower is integrated

#[cfg(feature = "std")]
use std::string::String;
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::String;

use wrt_foundation::values::Value;
#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use wrt_error::Result;

/// Component Model value types (re-exported for convenience)
/// These are high-level values that have been lifted from core WASM representation
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    /// Boolean
    Bool(bool),
    /// Signed 8-bit integer
    S8(i8),
    /// Unsigned 8-bit integer
    U8(u8),
    /// Signed 16-bit integer
    S16(i16),
    /// Unsigned 16-bit integer
    U16(u16),
    /// Signed 32-bit integer
    S32(i32),
    /// Unsigned 32-bit integer
    U32(u32),
    /// Signed 64-bit integer
    S64(i64),
    /// Unsigned 64-bit integer
    U64(u64),
    /// 32-bit float
    F32(f32),
    /// 64-bit float
    F64(f64),
    /// Unicode character
    Char(char),
    /// UTF-8 string (lifted from memory)
    String(String),
    /// List of values (lifted from memory)
    List(Vec<ComponentValue>),
    /// Record with named fields
    Record(Vec<(String, ComponentValue)>),
    /// Result type (ok or error)
    Result(core::result::Result<Option<Box<ComponentValue>>, Option<Box<ComponentValue>>>),
    /// Option type
    Option(Option<Box<ComponentValue>>),
    /// Resource handle (own or borrow)
    Handle(u32),
    /// Unit type (no value)
    Unit,
}

/// Resource handle type for wasip2 resources
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceHandle(pub u32);

/// Callback type for cabi_realloc invocation
///
/// Parameters: (old_ptr, old_size, align, new_size) -> Result<new_ptr>
#[cfg(feature = "std")]
pub type ReallocCallback<'a> = &'a mut dyn FnMut(i32, i32, i32, i32) -> Result<i32>;

/// Context for lowering ComponentValues to core WASM values
///
/// This provides access to memory and allocation facilities needed
/// for lowering complex types like strings and lists.
pub struct LoweringContext<'a> {
    /// Mutable reference to linear memory
    pub memory: &'a mut [u8],
    /// Callback to invoke cabi_realloc for memory allocation
    #[cfg(feature = "std")]
    pub realloc: Option<ReallocCallback<'a>>,
    /// Current allocation offset (for no_std fallback)
    pub alloc_offset: u32,
}

impl<'a> LoweringContext<'a> {
    /// Create a new lowering context with memory and optional realloc
    #[cfg(feature = "std")]
    pub fn new(memory: &'a mut [u8], realloc: Option<ReallocCallback<'a>>) -> Self {
        Self {
            memory,
            realloc,
            alloc_offset: 0,
        }
    }

    /// Create a new lowering context (no_std version)
    #[cfg(not(feature = "std"))]
    pub fn new(memory: &'a mut [u8]) -> Self {
        Self {
            memory,
            alloc_offset: 0,
        }
    }

    /// Allocate memory and return the pointer
    ///
    /// Uses cabi_realloc callback if available, otherwise falls back to
    /// simple bump allocation starting at alloc_offset.
    pub fn allocate(&mut self, size: u32, align: u32) -> Result<u32> {
        #[cfg(feature = "std")]
        if let Some(ref mut realloc) = self.realloc {
            // Call cabi_realloc(0, 0, align, size) for new allocation
            return realloc(0, 0, align as i32, size as i32).map(|p| p as u32);
        }

        // Fallback: bump allocation (for testing or no_std)
        // Align the offset
        let aligned = (self.alloc_offset + align - 1) & !(align - 1);
        let ptr = aligned;
        self.alloc_offset = aligned + size;

        if (self.alloc_offset as usize) > self.memory.len() {
            return Err(wrt_error::Error::runtime_error("Out of memory for lowering"));
        }

        Ok(ptr)
    }

    /// Write bytes to memory at the given offset
    pub fn write_bytes(&mut self, offset: u32, data: &[u8]) -> Result<()> {
        let start = offset as usize;
        let end = start + data.len();

        if end > self.memory.len() {
            return Err(wrt_error::Error::runtime_error("Memory write out of bounds"));
        }

        self.memory[start..end].copy_from_slice(data);
        Ok(())
    }
}

/// Component type for function signatures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasiComponentType {
    /// Primitive types - passed directly on stack
    Bool,
    U8,
    U16,
    U32,
    U64,
    S8,
    S16,
    S32,
    S64,
    F32,
    F64,
    Char,
    /// String - passed as ptr+len, needs lifting
    String,
    /// List<u8> - passed as ptr+len, needs lifting
    ListU8,
    /// List<T> - passed as ptr+len, needs lifting
    List(Box<WasiComponentType>),
    /// Option<T> - discriminant + optional value
    Option(Box<WasiComponentType>),
    /// Result<T, E> - discriminant + payload
    Result(Option<Box<WasiComponentType>>, Option<Box<WasiComponentType>>),
    /// Resource handle (own or borrow)
    Handle,
    /// Unit type (no value)
    Unit,
    /// Tuple of types
    Tuple(Vec<WasiComponentType>),
}

/// WASI function signature
#[derive(Debug, Clone)]
pub struct WasiFunctionSignature {
    /// Parameter types
    pub params: Vec<WasiComponentType>,
    /// Result types
    pub results: Vec<WasiComponentType>,
}

impl WasiFunctionSignature {
    fn new(params: Vec<WasiComponentType>, results: Vec<WasiComponentType>) -> Self {
        Self { params, results }
    }
}

/// Get the function signature for a WASI function.
/// Returns None if the function is unknown.
pub fn get_wasi_function_signature(interface: &str, function: &str) -> Option<WasiFunctionSignature> {
    // Strip version
    let base_interface = if let Some(at_pos) = interface.find('@') {
        &interface[..at_pos]
    } else {
        interface
    };

    match (base_interface, function) {
        // wasi:cli/stdout.get-stdout() -> own<output-stream>
        ("wasi:cli/stdout", "get-stdout") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::Handle],
        )),

        // wasi:cli/stderr.get-stderr() -> own<output-stream>
        ("wasi:cli/stderr", "get-stderr") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::Handle],
        )),

        // wasi:io/streams.[method]output-stream.blocking-write-and-flush
        // (self: borrow<output-stream>, contents: list<u8>) -> result<_, stream-error>
        ("wasi:io/streams", "[method]output-stream.blocking-write-and-flush") |
        ("wasi:io/streams", "output-stream.blocking-write-and-flush") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle, WasiComponentType::ListU8],
            vec![WasiComponentType::Result(None, Some(Box::new(WasiComponentType::String)))],
        )),

        // wasi:io/streams.[method]output-stream.blocking-flush
        // (self: borrow<output-stream>) -> result<_, stream-error>
        ("wasi:io/streams", "[method]output-stream.blocking-flush") |
        ("wasi:io/streams", "output-stream.blocking-flush") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle],
            vec![WasiComponentType::Result(None, Some(Box::new(WasiComponentType::String)))],
        )),

        // wasi:cli/exit.exit(status: result<_, _>)
        ("wasi:cli/exit", "exit") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Result(None, None)],
            vec![],
        )),

        // wasi:cli/environment.get-environment() -> list<tuple<string, string>>
        ("wasi:cli/environment", "get-environment") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::List(Box::new(WasiComponentType::Tuple(vec![
                WasiComponentType::String,
                WasiComponentType::String,
            ])))],
        )),

        // wasi:cli/environment.get-arguments() -> list<string>
        ("wasi:cli/environment", "get-arguments") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::List(Box::new(WasiComponentType::String))],
        )),

        // wasi:cli/environment.initial-cwd() -> option<string>
        ("wasi:cli/environment", "initial-cwd") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::Option(Box::new(WasiComponentType::String))],
        )),

        // Resource drops
        ("wasi:io/streams", "[resource-drop]output-stream") |
        ("wasi:io/streams", "[resource-drop]input-stream") |
        ("wasi:io/error", "[resource-drop]error") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle],
            vec![],
        )),

        // wasi:io/error.[method]error.to-debug-string
        ("wasi:io/error", "[method]error.to-debug-string") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle],
            vec![WasiComponentType::String],
        )),

        // wasi:cli/stdin.get-stdin() -> own<input-stream>
        ("wasi:cli/stdin", "get-stdin") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::Handle],
        )),

        // wasi:cli/terminal-stdin.get-terminal-stdin() -> option<own<terminal-input>>
        ("wasi:cli/terminal-stdin", "get-terminal-stdin") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::Option(Box::new(WasiComponentType::Handle))],
        )),

        // wasi:cli/terminal-stdout.get-terminal-stdout() -> option<own<terminal-output>>
        ("wasi:cli/terminal-stdout", "get-terminal-stdout") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::Option(Box::new(WasiComponentType::Handle))],
        )),

        // wasi:cli/terminal-stderr.get-terminal-stderr() -> option<own<terminal-output>>
        ("wasi:cli/terminal-stderr", "get-terminal-stderr") => Some(WasiFunctionSignature::new(
            vec![],
            vec![WasiComponentType::Option(Box::new(WasiComponentType::Handle))],
        )),

        // wasi:io/poll.poll(list<pollable>) -> list<u32>
        ("wasi:io/poll", "poll") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::List(Box::new(WasiComponentType::Handle))],
            vec![WasiComponentType::List(Box::new(WasiComponentType::U32))],
        )),

        // wasi:io/poll.[method]pollable.block(self: borrow<pollable>)
        ("wasi:io/poll", "[method]pollable.block") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle],
            vec![],
        )),

        // wasi:io/poll.[resource-drop]pollable
        ("wasi:io/poll", "[resource-drop]pollable") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle],
            vec![],
        )),

        // wasi:io/streams.[method]output-stream.subscribe() -> own<pollable>
        ("wasi:io/streams", "[method]output-stream.subscribe") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle],
            vec![WasiComponentType::Handle],
        )),

        // wasi:io/streams.[method]output-stream.check-write(self: borrow<output-stream>) -> result<u64, stream-error>
        ("wasi:io/streams", "[method]output-stream.check-write") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle],
            vec![WasiComponentType::Result(Some(Box::new(WasiComponentType::U64)), Some(Box::new(WasiComponentType::String)))],
        )),

        // wasi:io/streams.[method]output-stream.write(self: borrow<output-stream>, contents: list<u8>) -> result<_, stream-error>
        ("wasi:io/streams", "[method]output-stream.write") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle, WasiComponentType::ListU8],
            vec![WasiComponentType::Result(None, Some(Box::new(WasiComponentType::String)))],
        )),

        // Resource drops for terminal and poll resources
        ("wasi:cli/terminal-input", "[resource-drop]terminal-input") |
        ("wasi:cli/terminal-output", "[resource-drop]terminal-output") => Some(WasiFunctionSignature::new(
            vec![WasiComponentType::Handle],
            vec![],
        )),

        _ => None,
    }
}

/// Check if a WASI function needs canonical ABI lifting (has complex types)
pub fn wasi_function_needs_lifting(interface: &str, function: &str) -> bool {
    if let Some(sig) = get_wasi_function_signature(interface, function) {
        // Check if any param or result needs lifting
        sig.params.iter().any(|t| needs_lifting(t)) ||
        sig.results.iter().any(|t| needs_lifting(t))
    } else {
        // Unknown function - assume it might need lifting
        false
    }
}

fn needs_lifting(ty: &WasiComponentType) -> bool {
    match ty {
        WasiComponentType::String |
        WasiComponentType::ListU8 |
        WasiComponentType::List(_) => true,
        WasiComponentType::Option(inner) => needs_lifting(inner),
        WasiComponentType::Result(ok, err) => {
            ok.as_ref().map(|t| needs_lifting(t)).unwrap_or(false) ||
            err.as_ref().map(|t| needs_lifting(t)).unwrap_or(false)
        },
        WasiComponentType::Tuple(types) => types.iter().any(|t| needs_lifting(t)),
        _ => false,
    }
}

/// Lift core WASM values to ComponentValues using function signature.
///
/// This reads values from the WASM stack and memory, converting them
/// to proper Component Model values based on the function signature.
///
/// # Arguments
/// * `interface` - WASI interface name (e.g., "wasi:io/streams@0.2.0")
/// * `function` - Function name (e.g., "blocking-write-and-flush")
/// * `core_values` - Values from the WASM operand stack
/// * `memory` - Linear memory for reading strings/lists
///
/// # Returns
/// Vec of lifted ComponentValues ready for dispatch_component
pub fn lift_wasi_args(
    interface: &str,
    function: &str,
    core_values: &[Value],
    memory: Option<&[u8]>,
) -> Result<Vec<ComponentValue>> {
    let sig = get_wasi_function_signature(interface, function)
        .ok_or_else(|| wrt_error::Error::runtime_error("Unknown WASI function"))?;

    let mut result = Vec::new();
    let mut core_idx = 0;

    for param_ty in &sig.params {
        let (value, consumed) = lift_single_value(param_ty, &core_values[core_idx..], memory)?;
        result.push(value);
        core_idx += consumed;
    }

    Ok(result)
}

/// Lift a single value from core representation based on type
fn lift_single_value(
    ty: &WasiComponentType,
    core_values: &[Value],
    memory: Option<&[u8]>,
) -> Result<(ComponentValue, usize)> {
    match ty {
        WasiComponentType::Bool => {
            let v = get_i32(core_values, 0)?;
            Ok((ComponentValue::Bool(v != 0), 1))
        }
        WasiComponentType::U8 => {
            let v = get_i32(core_values, 0)?;
            Ok((ComponentValue::U8(v as u8), 1))
        }
        WasiComponentType::U16 => {
            let v = get_i32(core_values, 0)?;
            Ok((ComponentValue::U16(v as u16), 1))
        }
        WasiComponentType::U32 => {
            let v = get_i32(core_values, 0)?;
            Ok((ComponentValue::U32(v as u32), 1))
        }
        WasiComponentType::U64 => {
            let v = get_i64(core_values, 0)?;
            Ok((ComponentValue::U64(v as u64), 1))
        }
        WasiComponentType::S8 => {
            let v = get_i32(core_values, 0)?;
            Ok((ComponentValue::S8(v as i8), 1))
        }
        WasiComponentType::S16 => {
            let v = get_i32(core_values, 0)?;
            Ok((ComponentValue::S16(v as i16), 1))
        }
        WasiComponentType::S32 => {
            let v = get_i32(core_values, 0)?;
            Ok((ComponentValue::S32(v), 1))
        }
        WasiComponentType::S64 => {
            let v = get_i64(core_values, 0)?;
            Ok((ComponentValue::S64(v), 1))
        }
        WasiComponentType::F32 => {
            let v = get_f32(core_values, 0)?;
            Ok((ComponentValue::F32(v), 1))
        }
        WasiComponentType::F64 => {
            let v = get_f64(core_values, 0)?;
            Ok((ComponentValue::F64(v), 1))
        }
        WasiComponentType::Char => {
            let v = get_i32(core_values, 0)?;
            let ch = char::from_u32(v as u32)
                .ok_or_else(|| wrt_error::Error::runtime_error("Invalid char"))?;
            Ok((ComponentValue::Char(ch), 1))
        }
        WasiComponentType::Handle => {
            let v = get_i32(core_values, 0)?;
            Ok((ComponentValue::Handle(v as u32), 1))
        }
        WasiComponentType::Unit => {
            Ok((ComponentValue::Bool(true), 0)) // Unit consumes nothing
        }

        // String: ptr + len on stack, data in memory
        WasiComponentType::String => {
            let ptr = get_i32(core_values, 0)? as u32;
            let len = get_i32(core_values, 1)? as u32;

            let mem = memory.ok_or_else(||
                wrt_error::Error::runtime_error("Memory required for string lifting"))?;

            let start = ptr as usize;
            let end = start + len as usize;
            if end > mem.len() {
                return Err(wrt_error::Error::runtime_error("String out of bounds"));
            }

            let bytes = &mem[start..end];
            let s = core::str::from_utf8(bytes)
                .map_err(|_| wrt_error::Error::runtime_error("Invalid UTF-8"))?;

            Ok((ComponentValue::String(s.into()), 2))
        }

        // List<u8>: ptr + len on stack, bytes in memory
        WasiComponentType::ListU8 => {
            let ptr = get_i32(core_values, 0)? as u32;
            let len = get_i32(core_values, 1)? as u32;

            let mem = memory.ok_or_else(||
                wrt_error::Error::runtime_error("Memory required for list lifting"))?;

            let start = ptr as usize;
            let end = start + len as usize;
            if end > mem.len() {
                return Err(wrt_error::Error::runtime_error("List out of bounds"));
            }

            let bytes = &mem[start..end];
            let list: Vec<ComponentValue> = bytes.iter()
                .map(|&b| ComponentValue::U8(b))
                .collect();

            Ok((ComponentValue::List(list), 2))
        }

        // Generic List<T>: ptr + len on stack, elements in memory
        WasiComponentType::List(element_ty) => {
            let ptr = get_i32(core_values, 0)?;
            let len = get_i32(core_values, 1)?;

            // Empty list is always valid
            if len == 0 {
                return Ok((ComponentValue::List(Vec::new()), 2));
            }

            // For non-empty lists, we need memory access
            let mem = memory.ok_or_else(||
                wrt_error::Error::runtime_error("Memory required for list lifting"))?;

            // Calculate element size based on type
            let element_size = get_element_size(element_ty);

            let start = ptr as usize;
            let total_size = len as usize * element_size;
            let end = start + total_size;

            if end > mem.len() {
                return Err(wrt_error::Error::runtime_error("List out of bounds"));
            }

            // Lift each element from memory
            let mut elements = Vec::with_capacity(len as usize);
            for i in 0..len as usize {
                let offset = start + i * element_size;
                let element = lift_element_from_memory(element_ty, mem, offset)?;
                elements.push(element);
            }

            Ok((ComponentValue::List(elements), 2))
        }

        // Option<T>: discriminant + optional payload
        WasiComponentType::Option(inner_ty) => {
            let discriminant = get_i32(core_values, 0)?;
            if discriminant == 0 {
                Ok((ComponentValue::Option(None), 1))
            } else {
                let (inner, consumed) = lift_single_value(inner_ty, &core_values[1..], memory)?;
                Ok((ComponentValue::Option(Some(Box::new(inner))), 1 + consumed))
            }
        }

        // Result<T, E>: discriminant + payload
        WasiComponentType::Result(ok_ty, err_ty) => {
            let discriminant = get_i32(core_values, 0)?;
            if discriminant == 0 {
                // Ok case
                if let Some(ty) = ok_ty {
                    let (value, consumed) = lift_single_value(ty, &core_values[1..], memory)?;
                    Ok((ComponentValue::Result(Ok(Some(Box::new(value)))), 1 + consumed))
                } else {
                    Ok((ComponentValue::Result(Ok(None)), 1))
                }
            } else {
                // Err case
                if let Some(ty) = err_ty {
                    let (value, consumed) = lift_single_value(ty, &core_values[1..], memory)?;
                    Ok((ComponentValue::Result(Err(Some(Box::new(value)))), 1 + consumed))
                } else {
                    Ok((ComponentValue::Result(Err(None)), 1))
                }
            }
        }

        // Tuple: sequence of values
        WasiComponentType::Tuple(types) => {
            let mut values = Vec::new();
            let mut consumed = 0;
            for ty in types {
                let (value, c) = lift_single_value(ty, &core_values[consumed..], memory)?;
                values.push((format!("_{}", consumed), value));
                consumed += c;
            }
            Ok((ComponentValue::Record(values), consumed))
        }
    }
}

/// Lower ComponentValues to core WASM values based on function signature.
///
/// This converts Component Model values back to core WASM values
/// that can be pushed onto the operand stack.
///
/// # Arguments
/// * `interface` - WASI interface name
/// * `function` - Function name
/// * `component_values` - Component values from dispatch_component
/// * `memory` - Linear memory for writing strings/lists (mutable)
/// * `alloc_ptr` - Next free address in memory for allocation
///
/// # Returns
/// Vec of core Values to push onto WASM stack
pub fn lower_wasi_results(
    interface: &str,
    function: &str,
    component_values: &[ComponentValue],
    _memory: Option<&mut [u8]>,
    _alloc_ptr: u32,
) -> Result<Vec<Value>> {
    let sig = get_wasi_function_signature(interface, function)
        .ok_or_else(|| wrt_error::Error::runtime_error("Unknown WASI function"))?;

    let mut result = Vec::new();

    for (value, _ty) in component_values.iter().zip(sig.results.iter()) {
        lower_single_value(value, &mut result)?;
    }

    Ok(result)
}

/// Lower a single ComponentValue to core values
fn lower_single_value(value: &ComponentValue, out: &mut Vec<Value>) -> Result<()> {
    match value {
        ComponentValue::Bool(b) => {
            out.push(Value::I32(if *b { 1 } else { 0 }));
        }
        ComponentValue::U8(v) => out.push(Value::I32(*v as i32)),
        ComponentValue::U16(v) => out.push(Value::I32(*v as i32)),
        ComponentValue::U32(v) => out.push(Value::I32(*v as i32)),
        ComponentValue::U64(v) => out.push(Value::I64(*v as i64)),
        ComponentValue::S8(v) => out.push(Value::I32(*v as i32)),
        ComponentValue::S16(v) => out.push(Value::I32(*v as i32)),
        ComponentValue::S32(v) => out.push(Value::I32(*v)),
        ComponentValue::S64(v) => out.push(Value::I64(*v)),
        ComponentValue::F32(v) => out.push(Value::F32(wrt_foundation::float_repr::FloatBits32::from_f32(*v))),
        ComponentValue::F64(v) => out.push(Value::F64(wrt_foundation::float_repr::FloatBits64::from_f64(*v))),
        ComponentValue::Char(ch) => out.push(Value::I32(*ch as i32)),
        ComponentValue::Handle(h) => out.push(Value::I32(*h as i32)),

        // String: FAIL LOUD - proper lowering requires memory allocation via cabi_realloc
        ComponentValue::String(s) => {
            // Phase 4 TODO: Call cabi_realloc to allocate memory, write string, return ptr+len
            // For now, FAIL if we actually try to lower a non-empty string
            if !s.is_empty() {
                #[cfg(feature = "tracing")]
                wrt_foundation::tracing::warn!(len = s.len(), "String lowering not implemented");
                return Err(wrt_error::Error::runtime_error(
                    "String lowering requires memory allocation - not implemented"
                ));
            }
            // Empty string can be represented as (0, 0)
            out.push(Value::I32(0));
            out.push(Value::I32(0));
        }

        // List: FAIL LOUD - proper lowering requires memory allocation
        ComponentValue::List(items) => {
            // Phase 4 TODO: Allocate memory, recursively lower elements, return ptr+len
            if !items.is_empty() {
                #[cfg(feature = "tracing")]
                wrt_foundation::tracing::warn!(len = items.len(), "List lowering not implemented");
                return Err(wrt_error::Error::runtime_error(
                    "List lowering requires memory allocation - not implemented"
                ));
            }
            // Empty list can be represented as (0, 0)
            out.push(Value::I32(0));
            out.push(Value::I32(0));
        }

        // Record: flatten fields
        ComponentValue::Record(fields) => {
            for (_, field_value) in fields {
                lower_single_value(field_value, out)?;
            }
        }

        // Option: discriminant + optional payload
        ComponentValue::Option(None) => {
            out.push(Value::I32(0));
        }
        ComponentValue::Option(Some(inner)) => {
            out.push(Value::I32(1));
            lower_single_value(inner, out)?;
        }

        // Result: discriminant + payload
        ComponentValue::Result(Ok(None)) => {
            out.push(Value::I32(0));
        }
        ComponentValue::Result(Ok(Some(inner))) => {
            out.push(Value::I32(0));
            lower_single_value(inner, out)?;
        }
        ComponentValue::Result(Err(None)) => {
            out.push(Value::I32(1));
        }
        ComponentValue::Result(Err(Some(inner))) => {
            out.push(Value::I32(1));
            lower_single_value(inner, out)?;
        }

        // Unit: no values
        ComponentValue::Unit => {
            // Unit produces no values
        }
    }
    Ok(())
}

/// Lower WASI results with a context for memory allocation
///
/// This is the preferred lowering function when you have access to memory
/// and a realloc callback. It properly handles strings and lists by
/// allocating memory and writing the data.
pub fn lower_wasi_results_with_context(
    interface: &str,
    function: &str,
    component_values: &[ComponentValue],
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<Value>> {
    let sig = get_wasi_function_signature(interface, function)
        .ok_or_else(|| wrt_error::Error::runtime_error("Unknown WASI function"))?;

    let mut result = Vec::new();

    for (value, ty) in component_values.iter().zip(sig.results.iter()) {
        lower_single_value_with_context(value, ty, &mut result, ctx)?;
    }

    Ok(result)
}

/// Lower a single ComponentValue to core values with memory context
fn lower_single_value_with_context(
    value: &ComponentValue,
    ty: &WasiComponentType,
    out: &mut Vec<Value>,
    ctx: &mut LoweringContext<'_>,
) -> Result<()> {
    match (value, ty) {
        // Primitives - same as before, no memory needed
        (ComponentValue::Bool(b), _) => {
            out.push(Value::I32(if *b { 1 } else { 0 }));
        }
        (ComponentValue::U8(v), _) => out.push(Value::I32(*v as i32)),
        (ComponentValue::U16(v), _) => out.push(Value::I32(*v as i32)),
        (ComponentValue::U32(v), _) => out.push(Value::I32(*v as i32)),
        (ComponentValue::U64(v), _) => out.push(Value::I64(*v as i64)),
        (ComponentValue::S8(v), _) => out.push(Value::I32(*v as i32)),
        (ComponentValue::S16(v), _) => out.push(Value::I32(*v as i32)),
        (ComponentValue::S32(v), _) => out.push(Value::I32(*v)),
        (ComponentValue::S64(v), _) => out.push(Value::I64(*v)),
        (ComponentValue::F32(v), _) => out.push(Value::F32(wrt_foundation::float_repr::FloatBits32::from_f32(*v))),
        (ComponentValue::F64(v), _) => out.push(Value::F64(wrt_foundation::float_repr::FloatBits64::from_f64(*v))),
        (ComponentValue::Char(ch), _) => out.push(Value::I32(*ch as i32)),
        (ComponentValue::Handle(h), _) => out.push(Value::I32(*h as i32)),

        // String: Allocate memory, write UTF-8 bytes, return ptr+len
        (ComponentValue::String(s), _) => {
            if s.is_empty() {
                out.push(Value::I32(0));
                out.push(Value::I32(0));
            } else {
                let bytes = s.as_bytes();
                let len = bytes.len() as u32;
                // Allocate with alignment 1 (byte alignment)
                let ptr = ctx.allocate(len, 1)?;
                ctx.write_bytes(ptr, bytes)?;
                out.push(Value::I32(ptr as i32));
                out.push(Value::I32(len as i32));
            }
        }

        // List: Allocate memory, write elements, return ptr+len
        (ComponentValue::List(items), WasiComponentType::List(element_ty)) => {
            if items.is_empty() {
                out.push(Value::I32(0));
                out.push(Value::I32(0));
            } else {
                let element_size = get_element_size(element_ty) as u32;
                let total_size = items.len() as u32 * element_size;
                // Allocate with element alignment (at least 1)
                let align = core::cmp::max(1, element_size);
                let ptr = ctx.allocate(total_size, align)?;

                // Write each element
                for (i, item) in items.iter().enumerate() {
                    let offset = ptr + (i as u32 * element_size);
                    write_element_to_memory(item, element_ty, ctx.memory, offset as usize)?;
                }

                out.push(Value::I32(ptr as i32));
                out.push(Value::I32(items.len() as i32));
            }
        }

        // ListU8: Special case for byte arrays
        (ComponentValue::List(items), WasiComponentType::ListU8) => {
            if items.is_empty() {
                out.push(Value::I32(0));
                out.push(Value::I32(0));
            } else {
                let len = items.len() as u32;
                let ptr = ctx.allocate(len, 1)?;

                // Extract bytes from the list
                let bytes: Vec<u8> = items.iter().filter_map(|v| {
                    if let ComponentValue::U8(b) = v { Some(*b) } else { None }
                }).collect();

                if bytes.len() != items.len() {
                    return Err(wrt_error::Error::runtime_error("ListU8 contains non-byte elements"));
                }

                ctx.write_bytes(ptr, &bytes)?;
                out.push(Value::I32(ptr as i32));
                out.push(Value::I32(len as i32));
            }
        }

        // Record: flatten fields (recursive)
        (ComponentValue::Record(fields), WasiComponentType::Tuple(types)) => {
            for ((_, field_value), field_ty) in fields.iter().zip(types.iter()) {
                lower_single_value_with_context(field_value, field_ty, out, ctx)?;
            }
        }

        // Option: discriminant + optional payload
        (ComponentValue::Option(None), _) => {
            out.push(Value::I32(0));
        }
        (ComponentValue::Option(Some(inner)), WasiComponentType::Option(inner_ty)) => {
            out.push(Value::I32(1));
            lower_single_value_with_context(inner, inner_ty, out, ctx)?;
        }

        // Result: discriminant + payload
        (ComponentValue::Result(Ok(None)), _) => {
            out.push(Value::I32(0));
        }
        (ComponentValue::Result(Ok(Some(inner))), WasiComponentType::Result(Some(ok_ty), _)) => {
            out.push(Value::I32(0));
            lower_single_value_with_context(inner, ok_ty, out, ctx)?;
        }
        (ComponentValue::Result(Err(None)), _) => {
            out.push(Value::I32(1));
        }
        (ComponentValue::Result(Err(Some(inner))), WasiComponentType::Result(_, Some(err_ty))) => {
            out.push(Value::I32(1));
            lower_single_value_with_context(inner, err_ty, out, ctx)?;
        }

        // Unit: no values
        (ComponentValue::Unit, _) => {
            // Unit produces no values
        }

        // Fallback for mismatched types (delegate to simple lowering)
        _ => {
            lower_single_value(value, out)?;
        }
    }
    Ok(())
}

/// Write a single element to memory at the given offset
fn write_element_to_memory(
    value: &ComponentValue,
    ty: &WasiComponentType,
    memory: &mut [u8],
    offset: usize,
) -> Result<()> {
    let write_bytes = |mem: &mut [u8], off: usize, data: &[u8]| -> Result<()> {
        if off + data.len() > mem.len() {
            return Err(wrt_error::Error::runtime_error("Memory write out of bounds"));
        }
        mem[off..off + data.len()].copy_from_slice(data);
        Ok(())
    };

    match (value, ty) {
        (ComponentValue::Bool(b), _) => {
            write_bytes(memory, offset, &[if *b { 1 } else { 0 }])
        }
        (ComponentValue::U8(v), _) => write_bytes(memory, offset, &[*v]),
        (ComponentValue::S8(v), _) => write_bytes(memory, offset, &[*v as u8]),
        (ComponentValue::U16(v), _) => write_bytes(memory, offset, &v.to_le_bytes()),
        (ComponentValue::S16(v), _) => write_bytes(memory, offset, &v.to_le_bytes()),
        (ComponentValue::U32(v), _) => write_bytes(memory, offset, &v.to_le_bytes()),
        (ComponentValue::S32(v), _) => write_bytes(memory, offset, &v.to_le_bytes()),
        (ComponentValue::U64(v), _) => write_bytes(memory, offset, &v.to_le_bytes()),
        (ComponentValue::S64(v), _) => write_bytes(memory, offset, &v.to_le_bytes()),
        (ComponentValue::F32(v), _) => write_bytes(memory, offset, &v.to_bits().to_le_bytes()),
        (ComponentValue::F64(v), _) => write_bytes(memory, offset, &v.to_bits().to_le_bytes()),
        (ComponentValue::Char(ch), _) => write_bytes(memory, offset, &(*ch as u32).to_le_bytes()),
        (ComponentValue::Handle(h), _) => write_bytes(memory, offset, &h.to_le_bytes()),
        _ => Err(wrt_error::Error::runtime_error(
            "Cannot write complex type directly to memory"
        )),
    }
}

// Helper functions to extract values
fn get_i32(values: &[Value], idx: usize) -> Result<i32> {
    values.get(idx)
        .and_then(|v| match v {
            Value::I32(i) => Some(*i),
            _ => None,
        })
        .ok_or_else(|| wrt_error::Error::runtime_error("Expected i32"))
}

fn get_i64(values: &[Value], idx: usize) -> Result<i64> {
    values.get(idx)
        .and_then(|v| match v {
            Value::I64(i) => Some(*i),
            _ => None,
        })
        .ok_or_else(|| wrt_error::Error::runtime_error("Expected i64"))
}

fn get_f32(values: &[Value], idx: usize) -> Result<f32> {
    values.get(idx)
        .and_then(|v| match v {
            Value::F32(f) => Some(f.to_f32()),
            _ => None,
        })
        .ok_or_else(|| wrt_error::Error::runtime_error("Expected f32"))
}

fn get_f64(values: &[Value], idx: usize) -> Result<f64> {
    values.get(idx)
        .and_then(|v| match v {
            Value::F64(f) => Some(f.to_f64()),
            _ => None,
        })
        .ok_or_else(|| wrt_error::Error::runtime_error("Expected f64"))
}

/// Get the byte size of an element in memory for a given type
fn get_element_size(ty: &WasiComponentType) -> usize {
    match ty {
        WasiComponentType::Bool => 1,
        WasiComponentType::U8 | WasiComponentType::S8 => 1,
        WasiComponentType::U16 | WasiComponentType::S16 => 2,
        WasiComponentType::U32 | WasiComponentType::S32 => 4,
        WasiComponentType::U64 | WasiComponentType::S64 => 8,
        WasiComponentType::F32 => 4,
        WasiComponentType::F64 => 8,
        WasiComponentType::Char => 4, // UTF-32 code point
        WasiComponentType::String => 8, // ptr (4) + len (4)
        WasiComponentType::ListU8 => 8, // ptr (4) + len (4)
        WasiComponentType::List(_) => 8, // ptr (4) + len (4)
        WasiComponentType::Option(_) => 8, // discriminant + max(payload)
        WasiComponentType::Result(_, _) => 8, // discriminant + max(ok, err)
        WasiComponentType::Tuple(types) => {
            types.iter().map(|t| get_element_size(t)).sum()
        }
        WasiComponentType::Handle => 4, // u32 handle
        WasiComponentType::Unit => 0, // no size
    }
}

/// Lift a single element from memory at a given offset
fn lift_element_from_memory(
    ty: &WasiComponentType,
    memory: &[u8],
    offset: usize,
) -> Result<ComponentValue> {
    match ty {
        WasiComponentType::Bool => {
            if offset >= memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            Ok(ComponentValue::Bool(memory[offset] != 0))
        }
        WasiComponentType::U8 => {
            if offset >= memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            Ok(ComponentValue::U8(memory[offset]))
        }
        WasiComponentType::S8 => {
            if offset >= memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            Ok(ComponentValue::S8(memory[offset] as i8))
        }
        WasiComponentType::U16 => {
            if offset + 2 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let value = u16::from_le_bytes([memory[offset], memory[offset + 1]]);
            Ok(ComponentValue::U16(value))
        }
        WasiComponentType::S16 => {
            if offset + 2 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let value = i16::from_le_bytes([memory[offset], memory[offset + 1]]);
            Ok(ComponentValue::S16(value))
        }
        WasiComponentType::U32 => {
            if offset + 4 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let value = u32::from_le_bytes([
                memory[offset], memory[offset + 1],
                memory[offset + 2], memory[offset + 3],
            ]);
            Ok(ComponentValue::U32(value))
        }
        WasiComponentType::S32 => {
            if offset + 4 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let value = i32::from_le_bytes([
                memory[offset], memory[offset + 1],
                memory[offset + 2], memory[offset + 3],
            ]);
            Ok(ComponentValue::S32(value))
        }
        WasiComponentType::U64 => {
            if offset + 8 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let value = u64::from_le_bytes([
                memory[offset], memory[offset + 1], memory[offset + 2], memory[offset + 3],
                memory[offset + 4], memory[offset + 5], memory[offset + 6], memory[offset + 7],
            ]);
            Ok(ComponentValue::U64(value))
        }
        WasiComponentType::S64 => {
            if offset + 8 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let value = i64::from_le_bytes([
                memory[offset], memory[offset + 1], memory[offset + 2], memory[offset + 3],
                memory[offset + 4], memory[offset + 5], memory[offset + 6], memory[offset + 7],
            ]);
            Ok(ComponentValue::S64(value))
        }
        WasiComponentType::F32 => {
            if offset + 4 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let bits = u32::from_le_bytes([
                memory[offset], memory[offset + 1],
                memory[offset + 2], memory[offset + 3],
            ]);
            Ok(ComponentValue::F32(f32::from_bits(bits)))
        }
        WasiComponentType::F64 => {
            if offset + 8 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let bits = u64::from_le_bytes([
                memory[offset], memory[offset + 1], memory[offset + 2], memory[offset + 3],
                memory[offset + 4], memory[offset + 5], memory[offset + 6], memory[offset + 7],
            ]);
            Ok(ComponentValue::F64(f64::from_bits(bits)))
        }
        WasiComponentType::Char => {
            if offset + 4 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let code = u32::from_le_bytes([
                memory[offset], memory[offset + 1],
                memory[offset + 2], memory[offset + 3],
            ]);
            let ch = char::from_u32(code)
                .ok_or_else(|| wrt_error::Error::runtime_error("Invalid char code point"))?;
            Ok(ComponentValue::Char(ch))
        }
        WasiComponentType::String => {
            // String is ptr + len in memory
            if offset + 8 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let ptr = u32::from_le_bytes([
                memory[offset], memory[offset + 1],
                memory[offset + 2], memory[offset + 3],
            ]) as usize;
            let len = u32::from_le_bytes([
                memory[offset + 4], memory[offset + 5],
                memory[offset + 6], memory[offset + 7],
            ]) as usize;

            if len == 0 {
                return Ok(ComponentValue::String(String::new()));
            }

            if ptr + len > memory.len() {
                return Err(wrt_error::Error::runtime_error("String data out of bounds"));
            }

            let bytes = &memory[ptr..ptr + len];
            let s = core::str::from_utf8(bytes)
                .map_err(|_| wrt_error::Error::runtime_error("Invalid UTF-8 in string"))?;
            Ok(ComponentValue::String(s.to_string()))
        }
        WasiComponentType::Handle => {
            if offset + 4 > memory.len() {
                return Err(wrt_error::Error::runtime_error("Memory access out of bounds"));
            }
            let value = u32::from_le_bytes([
                memory[offset], memory[offset + 1],
                memory[offset + 2], memory[offset + 3],
            ]);
            Ok(ComponentValue::Handle(value))
        }
        // Unit has no size, nothing to read
        WasiComponentType::Unit => Ok(ComponentValue::Unit),
        // For complex types, we would need recursive lifting
        WasiComponentType::ListU8 | WasiComponentType::List(_) |
        WasiComponentType::Option(_) | WasiComponentType::Result(_, _) |
        WasiComponentType::Tuple(_) => {
            Err(wrt_error::Error::runtime_error(
                "Nested complex types in lists not yet supported"
            ))
        }
    }
}

/// Output stream resource for wasi:io/streams
pub struct OutputStreamResource {
    /// The actual output destination (stdout, stderr, file, etc.)
    pub target: OutputTarget,
    /// Buffer for pending writes
    pub buffer: Vec<u8>,
}

/// Output target for streams
#[derive(Debug, Clone)]
pub enum OutputTarget {
    Stdout,
    Stderr,
    File(String),
    Memory(Vec<u8>),
}

/// Error resource for wasi:io/error
pub struct ErrorResource {
    pub message: String,
    pub code: u32,
}

/// WASI Preview2 Host Functions
pub struct Wasip2Host {
    /// Next available resource handle
    next_handle: u32,
    /// Output stream resources
    output_streams: Vec<OutputStreamResource>,
    /// Error resources
    errors: Vec<ErrorResource>,
}

impl Default for Wasip2Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Wasip2Host {
    pub fn new() -> Self {
        let mut host = Self {
            next_handle: 1,
            output_streams: Vec::new(),
            errors: Vec::new(),
        };

        // Pre-create stdout and stderr streams (handles 1 and 2)
        host.create_stdout();
        host.create_stderr();

        host
    }

    fn create_stdout(&mut self) -> ResourceHandle {
        let stream = OutputStreamResource {
            target: OutputTarget::Stdout,
            buffer: Vec::new(),
        };
        self.output_streams.push(stream);
        let handle = ResourceHandle(self.next_handle);
        self.next_handle += 1;
        handle
    }

    fn create_stderr(&mut self) -> ResourceHandle {
        let stream = OutputStreamResource {
            target: OutputTarget::Stderr,
            buffer: Vec::new(),
        };
        self.output_streams.push(stream);
        let handle = ResourceHandle(self.next_handle);
        self.next_handle += 1;
        handle
    }

    /// wasi:cli/stdout.get-stdout
    pub fn cli_stdout_get_stdout(&mut self) -> Result<Vec<Value>> {
        // Return handle to stdout stream (always handle 1)
        Ok(vec![Value::I32(1)])
    }

    /// wasi:cli/stderr.get-stderr
    pub fn cli_stderr_get_stderr(&mut self) -> Result<Vec<Value>> {
        // Return handle to stderr stream (always handle 2)
        Ok(vec![Value::I32(2)])
    }

    /// wasi:io/streams.output-stream.blocking-write-and-flush
    pub fn io_streams_output_stream_blocking_write_and_flush(
        &mut self,
        handle: u32,
        data_ptr: u32,
        data_len: u32,
        memory: &mut [u8],
    ) -> Result<Vec<Value>> {
        // Get the stream resource
        let stream_idx = (handle - 1) as usize;
        if stream_idx >= self.output_streams.len() {
            return Ok(vec![Value::I32(1)]); // Return error variant
        }

        // Read data from memory
        let start = data_ptr as usize;
        let end = start + data_len as usize;
        if end > memory.len() {
            return Ok(vec![Value::I32(1)]); // Return error variant
        }

        let data = &memory[start..end];

        // Find the actual content length - some components pass buffer capacity
        // instead of actual content length. For text output, trim at null byte.
        let actual_len = data.iter()
            .position(|&b| b == 0)
            .unwrap_or(data.len());
        let trimmed_data = &data[..actual_len];

        #[cfg(feature = "tracing")]
        wrt_foundation::tracing::trace!(
            handle = handle,
            ptr = data_ptr,
            len = data_len,
            actual_len = actual_len,
            data = %String::from_utf8_lossy(trimmed_data),
            "WASIP2 write"
        );

        // Write to the appropriate target
        match &self.output_streams[stream_idx].target {
            OutputTarget::Stdout => {
                #[cfg(feature = "std")]
                {
                    use std::io::{self, Write};
                    #[cfg(feature = "tracing")]
                    wrt_foundation::tracing::trace!(bytes = trimmed_data.len(), "Writing to STDOUT");
                    let _ = io::stdout().write_all(trimmed_data);
                    let _ = io::stdout().flush();
                }
            },
            OutputTarget::Stderr => {
                #[cfg(feature = "std")]
                {
                    use std::io::{self, Write};
                    let _ = io::stderr().write_all(trimmed_data);
                    let _ = io::stderr().flush();
                }
            },
            OutputTarget::File(_path) => {
                // File output not yet implemented
            },
            OutputTarget::Memory(_buffer) => {
                // In a real implementation, we'd append to the buffer
            }
        }

        // Return ok variant (0) with no error
        Ok(vec![Value::I32(0)])
    }

    /// wasi:io/streams.output-stream.blocking-flush
    pub fn io_streams_output_stream_blocking_flush(
        &mut self,
        _handle: u32,
    ) -> Result<Vec<Value>> {
        // For now, we don't buffer, so flush is a no-op
        // Return ok variant (0)
        Ok(vec![Value::I32(0)])
    }

    /// wasi:cli/exit.exit
    /// This is called when the component wants to exit.
    /// The status parameter indicates success (ok variant) or failure.
    pub fn cli_exit(&mut self, status_ok: bool) -> Result<Vec<Value>> {
        #[cfg(feature = "std")]
        {
            // In std mode, actually exit the process
            let code = if status_ok { 0 } else { 1 };
            std::process::exit(code);
        }
        #[cfg(not(feature = "std"))]
        {
            if status_ok {
                Ok(vec![])
            } else {
                Err(wrt_error::Error::runtime_error("Component exited with failure status"))
            }
        }
    }

    /// Extract the base interface name without version suffix
    /// e.g., "wasi:cli/stdout@0.2.4" -> "wasi:cli/stdout"
    fn strip_version(interface: &str) -> &str {
        if let Some(idx) = interface.find('@') {
            &interface[..idx]
        } else {
            interface
        }
    }

    /// Dispatch a wasip2 function call based on interface and function name
    pub fn dispatch(
        &mut self,
        interface: &str,
        function: &str,
        args: Vec<Value>,
        memory: Option<&mut [u8]>,
    ) -> Result<Vec<Value>> {
        // Strip version for matching - accept any 0.2.x version
        let base_interface = Self::strip_version(interface);

        match (base_interface, function) {
            ("wasi:cli/stdout", "get-stdout") => {
                self.cli_stdout_get_stdout()
            },
            ("wasi:cli/stderr", "get-stderr") => {
                self.cli_stderr_get_stderr()
            },
            ("wasi:io/streams", "[method]output-stream.blocking-write-and-flush") |
            ("wasi:io/streams", "output-stream.blocking-write-and-flush") => {
                if args.len() < 3 {
                    return Err(wrt_error::Error::runtime_error("Invalid args for blocking-write-and-flush"));
                }
                let handle = match args[0] {
                    Value::I32(h) => h as u32,
                    _ => return Err(wrt_error::Error::runtime_error("Invalid handle type")),
                };
                let data_ptr = match args[1] {
                    Value::I32(p) => p as u32,
                    _ => return Err(wrt_error::Error::runtime_error("Invalid ptr type")),
                };
                let data_len = match args[2] {
                    Value::I32(l) => l as u32,
                    _ => return Err(wrt_error::Error::runtime_error("Invalid len type")),
                };

                if let Some(mem) = memory {
                    self.io_streams_output_stream_blocking_write_and_flush(handle, data_ptr, data_len, mem)
                } else {
                    Err(wrt_error::Error::runtime_error("No memory for blocking-write-and-flush"))
                }
            },
            ("wasi:io/streams", "[method]output-stream.blocking-flush") |
            ("wasi:io/streams", "output-stream.blocking-flush") => {
                if args.is_empty() {
                    return Err(wrt_error::Error::runtime_error("Invalid args for blocking-flush"));
                }
                let handle = match args[0] {
                    Value::I32(h) => h as u32,
                    _ => return Err(wrt_error::Error::runtime_error("Invalid handle type")),
                };
                self.io_streams_output_stream_blocking_flush(handle)
            },
            ("wasi:cli/exit", "exit") => {
                // Exit takes a result<_, _> as parameter
                // If the first arg is 0, it's the ok variant (success)
                // If the first arg is 1, it's the error variant (failure)
                let status_ok = match args.first() {
                    Some(Value::I32(0)) => true,  // ok variant
                    Some(Value::I32(1)) => false, // error variant
                    _ => true, // Default to success if no args or unknown
                };
                self.cli_exit(status_ok)
            },
            ("wasi:cli/stdin", "get-stdin") => {
                // Return a resource handle for stdin (input-stream)
                Ok(vec![Value::I32(0)]) // Handle 0 = stdin
            },
            ("wasi:cli/terminal-stdin", "get-terminal-stdin") => {
                // Return Option<terminal-input>: 0 = None (not a terminal)
                // For now, return none (0,0) - first 0 is discriminant for None
                Ok(vec![Value::I32(0)])
            },
            ("wasi:cli/terminal-stdout", "get-terminal-stdout") => {
                // Return Option<terminal-output>: 0 = None (not a terminal)
                Ok(vec![Value::I32(0)])
            },
            ("wasi:cli/terminal-stderr", "get-terminal-stderr") => {
                // Return Option<terminal-output>: 0 = None (not a terminal)
                Ok(vec![Value::I32(0)])
            },
            ("wasi:io/poll", "poll") |
            ("wasi:io/poll", "[method]pollable.block") => {
                // Simple poll implementation - return immediately with all ready
                // Args: list of pollable handles
                // Returns: list of ready indices
                let ready_indices: Vec<Value> = vec![Value::I32(0)]; // First pollable always ready
                Ok(ready_indices)
            },
            ("wasi:io/poll", "[resource-drop]pollable") |
            ("wasi:io/streams", "[resource-drop]output-stream") |
            ("wasi:io/streams", "[resource-drop]input-stream") |
            ("wasi:io/error", "[resource-drop]error") |
            ("wasi:cli/terminal-input", "[resource-drop]terminal-input") |
            ("wasi:cli/terminal-output", "[resource-drop]terminal-output") => {
                // Resource drops are no-ops in this simple implementation
                Ok(vec![])
            },
            ("wasi:io/streams", "[method]output-stream.check-write") => {
                // Check how much we can write - return Result<u64, stream-error>
                // Return Ok(1MB) - we can write 1MB at a time
                Ok(vec![Value::I32(0), Value::I64(1024 * 1024)]) // discriminant 0 = ok, value = 1MB
            },
            ("wasi:io/streams", "[method]output-stream.write") => {
                // Write to output stream - simplified version
                // Returns Result<_, stream-error>
                if args.len() >= 3 {
                    if let Some(mem) = memory {
                        let handle = match args[0] {
                            Value::I32(h) => h as u32,
                            _ => return Err(wrt_error::Error::runtime_error("Invalid handle")),
                        };
                        let data_ptr = match args[1] {
                            Value::I32(p) => p as u32,
                            _ => return Err(wrt_error::Error::runtime_error("Invalid ptr")),
                        };
                        let data_len = match args[2] {
                            Value::I32(l) => l as u32,
                            _ => return Err(wrt_error::Error::runtime_error("Invalid len")),
                        };
                        return self.io_streams_output_stream_blocking_write_and_flush(handle, data_ptr, data_len, mem);
                    }
                }
                // Return success (discriminant 0)
                Ok(vec![Value::I32(0)])
            },
            ("wasi:io/streams", "[method]output-stream.subscribe") => {
                // Subscribe returns a pollable handle
                // We use a simple scheme: pollable = stream_handle + 1000
                let handle = match args.first() {
                    Some(Value::I32(h)) => *h as u32,
                    _ => 0,
                };
                Ok(vec![Value::I32((handle + 1000) as i32)])
            },
            _ => {
                #[cfg(feature = "std")]
                eprintln!("[WASIP2] Unknown function: {}/{}", base_interface, function);
                Err(wrt_error::Error::runtime_error("Unknown wasip2 function"))
            }
        }
    }

    // ========================================================================
    // COMPONENT MODEL INTERFACE
    // ========================================================================
    // These methods work with ComponentValue instead of raw Value + memory.
    // Complex types (strings, lists) are first-class values, already lifted.

    /// Dispatch a WASI call using Component Model values.
    ///
    /// This is the **proper** Component Model interface where:
    /// - Strings are `ComponentValue::String`, not ptr+len
    /// - Lists are `ComponentValue::List`, not ptr+len
    /// - Results are `ComponentValue::Result`, not discriminant+payload
    ///
    /// This should be called AFTER canonical ABI lifting has converted
    /// core WASM values to component values.
    pub fn dispatch_component(
        &mut self,
        interface: &str,
        function: &str,
        args: Vec<ComponentValue>,
    ) -> Result<Vec<ComponentValue>> {
        let base_interface = Self::strip_version(interface);

        match (base_interface, function) {
            // wasi:cli/stdout.get-stdout() -> own<output-stream>
            ("wasi:cli/stdout", "get-stdout") => {
                Ok(vec![ComponentValue::Handle(1)]) // stdout handle
            },

            // wasi:cli/stderr.get-stderr() -> own<output-stream>
            ("wasi:cli/stderr", "get-stderr") => {
                Ok(vec![ComponentValue::Handle(2)]) // stderr handle
            },

            // wasi:io/streams.[method]output-stream.blocking-write-and-flush
            // (self: borrow<output-stream>, contents: list<u8>) -> result<_, stream-error>
            ("wasi:io/streams", "[method]output-stream.blocking-write-and-flush") |
            ("wasi:io/streams", "output-stream.blocking-write-and-flush") => {
                // Args: [handle, list<u8>]
                if args.len() < 2 {
                    return Err(wrt_error::Error::runtime_error(
                        "blocking-write-and-flush requires 2 args: handle, contents"
                    ));
                }

                let handle = match &args[0] {
                    ComponentValue::Handle(h) => *h,
                    ComponentValue::U32(h) => *h,
                    ComponentValue::S32(h) => *h as u32,
                    _ => return Err(wrt_error::Error::runtime_error("First arg must be handle")),
                };

                // Get the data - either as a lifted list<u8> or as a string
                let data: Vec<u8> = match &args[1] {
                    ComponentValue::List(items) => {
                        items.iter().filter_map(|v| match v {
                            ComponentValue::U8(b) => Some(*b),
                            ComponentValue::S8(b) => Some(*b as u8),
                            _ => None,
                        }).collect()
                    },
                    ComponentValue::String(s) => s.as_bytes().to_vec(),
                    _ => return Err(wrt_error::Error::runtime_error("Second arg must be list<u8> or string")),
                };

                // Perform the write
                let result = self.write_to_stream(handle, &data);

                // Return result<_, stream-error>
                match result {
                    Ok(()) => Ok(vec![ComponentValue::Result(Ok(None))]),
                    Err(e) => Ok(vec![ComponentValue::Result(Err(Some(Box::new(
                        ComponentValue::String(e.to_string())
                    ))))]),
                }
            },

            // wasi:io/streams.[method]output-stream.blocking-flush
            // (self: borrow<output-stream>) -> result<_, stream-error>
            ("wasi:io/streams", "[method]output-stream.blocking-flush") |
            ("wasi:io/streams", "output-stream.blocking-flush") => {
                // Flush is a no-op for now
                Ok(vec![ComponentValue::Result(Ok(None))])
            },

            // wasi:cli/exit.exit(status: result<_, _>)
            ("wasi:cli/exit", "exit") => {
                let success = match args.first() {
                    Some(ComponentValue::Result(Ok(_))) => true,
                    Some(ComponentValue::Result(Err(_))) => false,
                    Some(ComponentValue::S32(0)) => true,  // Legacy: 0 = ok variant
                    Some(ComponentValue::S32(1)) => false, // Legacy: 1 = err variant
                    _ => true,
                };

                #[cfg(feature = "std")]
                {
                    std::process::exit(if success { 0 } else { 1 });
                }
                #[cfg(not(feature = "std"))]
                {
                    if success {
                        Ok(vec![])
                    } else {
                        Err(wrt_error::Error::runtime_error("Component exited with failure"))
                    }
                }
            },

            // wasi:cli/environment.get-environment() -> list<tuple<string, string>>
            ("wasi:cli/environment", "get-environment") => {
                // Return empty environment for now
                Ok(vec![ComponentValue::List(Vec::new())])
            },

            // wasi:cli/environment.get-arguments() -> list<string>
            ("wasi:cli/environment", "get-arguments") => {
                // Return empty arguments for now
                Ok(vec![ComponentValue::List(Vec::new())])
            },

            // wasi:cli/environment.initial-cwd() -> option<string>
            ("wasi:cli/environment", "initial-cwd") => {
                Ok(vec![ComponentValue::Option(None)])
            },

            // Resource drops
            ("wasi:io/streams", "[resource-drop]output-stream") |
            ("wasi:io/streams", "[resource-drop]input-stream") |
            ("wasi:io/error", "[resource-drop]error") => {
                // No-op for now
                Ok(vec![])
            },

            // wasi:io/error.[method]error.to-debug-string
            ("wasi:io/error", "[method]error.to-debug-string") => {
                Ok(vec![ComponentValue::String("error".into())])
            },

            _ => {
                #[cfg(feature = "tracing")]
                wrt_foundation::tracing::warn!(interface = interface, function = function, "Unknown WASIP2 component function");
                Err(wrt_error::Error::runtime_error("Unknown wasip2 component function"))
            }
        }
    }

    /// Write data to a stream (internal helper)
    fn write_to_stream(&mut self, handle: u32, data: &[u8]) -> Result<()> {
        let stream_idx = (handle - 1) as usize;
        if stream_idx >= self.output_streams.len() {
            return Err(wrt_error::Error::runtime_error("Invalid stream handle"));
        }

        match &self.output_streams[stream_idx].target {
            OutputTarget::Stdout => {
                #[cfg(feature = "std")]
                {
                    use std::io::{self, Write};
                    io::stdout().write_all(data).map_err(|_|
                        wrt_error::Error::runtime_error("stdout write failed"))?;
                    io::stdout().flush().map_err(|_|
                        wrt_error::Error::runtime_error("stdout flush failed"))?;
                }
                Ok(())
            },
            OutputTarget::Stderr => {
                #[cfg(feature = "std")]
                {
                    use std::io::{self, Write};
                    io::stderr().write_all(data).map_err(|_|
                        wrt_error::Error::runtime_error("stderr write failed"))?;
                    io::stderr().flush().map_err(|_|
                        wrt_error::Error::runtime_error("stderr flush failed"))?;
                }
                Ok(())
            },
            OutputTarget::File(_) => {
                Err(wrt_error::Error::runtime_error("File output not implemented"))
            },
            OutputTarget::Memory(_) => {
                // Would append to buffer
                Ok(())
            }
        }
    }
}