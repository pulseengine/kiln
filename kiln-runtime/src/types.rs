//! Type aliases for `no_std` compatibility in wrt-runtime

use wrt_foundation::{
    BoundedMap,
    BoundedVec,
};

use crate::prelude::*;

/// Platform-aware memory provider for runtime types
pub(crate) type RuntimeProvider = crate::bounded_runtime_infra::RuntimeProvider;

// Runtime execution limits
/// Maximum recursion depth for function calls
pub const MAX_STACK_DEPTH: usize = 1024;
/// Maximum number of frames in the call stack
pub const MAX_CALL_STACK: usize = 512;
/// Maximum number of values on the value stack
pub const MAX_VALUE_STACK: usize = 65536;
/// Maximum number of local variables per function (WebAssembly spec limit)
pub const MAX_LOCALS: usize = 50000;
/// Maximum number of global variables
pub const MAX_GLOBALS: usize = 1024;
/// Maximum number of functions in a module
pub const MAX_FUNCTIONS: usize = 1024;
/// Maximum number of imports in a module
pub const MAX_IMPORTS: usize = 512;
/// Maximum number of exports in a module
pub const MAX_EXPORTS: usize = 512;
/// Maximum number of tables in a module
pub const MAX_TABLES: usize = 64;
/// Maximum number of memories in a module
pub const MAX_MEMORIES: usize = 64;
/// Maximum number of element segments
pub const MAX_ELEMENTS: usize = 512;
/// Maximum number of data segments
pub const MAX_DATA: usize = 512;

// Memory management
/// Maximum number of 64KB memory pages (4GB total)
pub const MAX_MEMORY_PAGES: usize = 65536;
/// Maximum number of entries in a table (1M entries)
pub const MAX_TABLE_ENTRIES: usize = 1048576;
/// Maximum length for string values
pub const MAX_STRING_LENGTH: usize = 256;

// Module instance limits
/// Maximum number of module instances
pub const MAX_MODULE_INSTANCES: usize = 256;
/// Maximum number of function bodies
pub const MAX_FUNCTION_BODIES: usize = 1024;
/// Maximum number of branch table targets
pub const MAX_BRANCH_TABLE_TARGETS: usize = 1024;

// CFI and instrumentation
/// Maximum number of CFI checks per function
pub const MAX_CFI_CHECKS: usize = 1024;
/// Maximum number of instrumentation points
pub const MAX_INSTRUMENTATION_POINTS: usize = 2048;

// Runtime state vectors
/// Value stack type for std environments
#[cfg(feature = "std")]
pub type ValueStackVec = Vec<wrt_foundation::Value>;
/// Value stack type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type ValueStackVec = BoundedVec<wrt_foundation::Value, MAX_VALUE_STACK, RuntimeProvider>;

/// Call stack type for std environments
#[cfg(feature = "std")]
pub type CallStackVec = Vec<crate::core_types::CallFrame>;
/// Call stack type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type CallStackVec = BoundedVec<crate::core_types::CallFrame, MAX_CALL_STACK, RuntimeProvider>;

/// Local variables vector type for std environments
#[cfg(feature = "std")]
pub type LocalsVec = Vec<wrt_foundation::Value>;
/// Local variables vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type LocalsVec = BoundedVec<wrt_foundation::Value, MAX_LOCALS, RuntimeProvider>;

/// Global variables vector type for std environments
#[cfg(feature = "std")]
pub type GlobalsVec = Vec<crate::global::Global>;
/// Global variables vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type GlobalsVec = BoundedVec<crate::global::Global, MAX_GLOBALS, RuntimeProvider>;

/// Functions vector type for std environments
#[cfg(feature = "std")]
pub type FunctionsVec = Vec<crate::func::Function>;
/// Functions vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type FunctionsVec = BoundedVec<crate::func::Function, MAX_FUNCTIONS, RuntimeProvider>;

/// Imports vector type for std environments
#[cfg(feature = "std")]
pub type ImportsVec<T> = Vec<T>;
/// Imports vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type ImportsVec<T> = BoundedVec<T, MAX_IMPORTS, RuntimeProvider>;

/// Exports vector type for std environments
#[cfg(feature = "std")]
pub type ExportsVec<T> = Vec<T>;
/// Exports vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type ExportsVec<T> = BoundedVec<T, MAX_EXPORTS, RuntimeProvider>;

/// Tables vector type for std environments
#[cfg(feature = "std")]
pub type TablesVec = Vec<crate::table::Table>;
/// Tables vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type TablesVec = BoundedVec<crate::table::Table, MAX_TABLES, RuntimeProvider>;

/// Memories vector type for std environments
#[cfg(feature = "std")]
pub type MemoriesVec = Vec<crate::memory::Memory>;
/// Memories vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type MemoriesVec = BoundedVec<crate::memory::Memory, MAX_MEMORIES, RuntimeProvider>;

/// Element segments vector type for std environments
#[cfg(feature = "std")]
pub type ElementsVec = Vec<wrt_foundation::types::ElementSegment>;
/// Element segments vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type ElementsVec =
    BoundedVec<wrt_foundation::types::ElementSegment, MAX_ELEMENTS, RuntimeProvider>;

/// Data segments vector type for std environments
#[cfg(feature = "std")]
pub type DataVec = Vec<wrt_foundation::types::DataSegment>;
/// Data segments vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type DataVec = BoundedVec<wrt_foundation::types::DataSegment, MAX_DATA, RuntimeProvider>;

// Instruction vectors
/// Instructions vector type for std environments
#[cfg(feature = "std")]
// Instructions module is temporarily disabled in wrt-decoder
// pub type InstructionVec = Vec<wrt_decoder::instructions::Instruction>;
pub type InstructionVec = Vec<crate::prelude::Instruction>;
/// Instructions vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type InstructionVec = BoundedVec<crate::prelude::Instruction, 65536, RuntimeProvider>;

/// Branch targets vector type for std environments
#[cfg(feature = "std")]
pub type BranchTargetsVec = Vec<u32>;
/// Branch targets vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type BranchTargetsVec = BoundedVec<u32, MAX_BRANCH_TABLE_TARGETS, RuntimeProvider>;

// Module instance vectors
/// Module instances vector type for std environments
#[cfg(feature = "std")]
pub type ModuleInstanceVec = Vec<crate::module_instance::ModuleInstance>;
/// Module instances vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type ModuleInstanceVec =
    BoundedVec<crate::module_instance::ModuleInstance, MAX_MODULE_INSTANCES, RuntimeProvider>;

/// Function bodies vector type for std environments
#[cfg(feature = "std")]
pub type FunctionBodiesVec = Vec<Vec<u8>>;
/// Function bodies vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type FunctionBodiesVec =
    BoundedVec<BoundedVec<u8, 65536, RuntimeProvider>, MAX_FUNCTION_BODIES, RuntimeProvider>;

// Memory and table data
/// Memory data vector type for std environments
#[cfg(feature = "std")]
pub type MemoryDataVec = Vec<u8>;
/// Memory data vector type for `no_std` environments (64MB max)
#[cfg(not(feature = "std"))]
pub type MemoryDataVec = BoundedVec<u8, { 64 * 1024 * 1024 }, RuntimeProvider>;

/// Table data vector type for std environments
#[cfg(feature = "std")]
pub type TableDataVec = Vec<Option<crate::prelude::RefValue>>;
/// Table data vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type TableDataVec =
    BoundedVec<Option<crate::prelude::RefValue>, MAX_TABLE_ENTRIES, RuntimeProvider>;

// String type for runtime
/// Runtime string type for std environments
#[cfg(feature = "std")]
pub type RuntimeString = String;
/// Runtime string type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type RuntimeString = wrt_foundation::BoundedString<MAX_STRING_LENGTH>;

// Maps for runtime state
/// Function map type for std environments
#[cfg(feature = "std")]
pub type FunctionMap = HashMap<u32, crate::func::Function>;
/// Function map type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type FunctionMap = BoundedMap<u32, crate::func::Function, MAX_FUNCTIONS, RuntimeProvider>;

/// Global map type for std environments
#[cfg(feature = "std")]
pub type GlobalMap = HashMap<u32, crate::global::Global>;
/// Global map type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type GlobalMap = BoundedMap<u32, crate::global::Global, MAX_GLOBALS, RuntimeProvider>;

/// Memory map type for std environments
#[cfg(feature = "std")]
pub type MemoryMap = HashMap<u32, crate::memory::Memory>;
/// Memory map type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type MemoryMap = BoundedMap<u32, crate::memory::Memory, MAX_MEMORIES, RuntimeProvider>;

/// Table map type for std environments
#[cfg(feature = "std")]
pub type TableMap = HashMap<u32, crate::table::Table>;
/// Table map type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type TableMap = BoundedMap<u32, crate::table::Table, MAX_TABLES, RuntimeProvider>;

// CFI and instrumentation types
/// CFI checks vector type for std environments
#[cfg(feature = "std")]
pub type CfiCheckVec = Vec<crate::cfi_engine::CfiCheck>;
/// CFI checks vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type CfiCheckVec = BoundedVec<crate::cfi_engine::CfiCheck, MAX_CFI_CHECKS, RuntimeProvider>;

/// Instrumentation points vector type for std environments
#[cfg(feature = "std")]
pub type InstrumentationVec = Vec<crate::execution::InstrumentationPoint>;
/// Instrumentation points vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type InstrumentationVec =
    BoundedVec<crate::execution::InstrumentationPoint, MAX_INSTRUMENTATION_POINTS, RuntimeProvider>;

// Generic byte vector for raw data
/// Byte vector type for std environments
#[cfg(feature = "std")]
pub type ByteVec = Vec<u8>;
/// Byte vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type ByteVec = BoundedVec<u8, 65536, RuntimeProvider>;

// Error collection for batch operations
/// Error vector type for std environments
#[cfg(feature = "std")]
pub type ErrorVec = Vec<wrt_error::Error>;
/// Error vector type for `no_std` environments
#[cfg(not(feature = "std"))]
pub type ErrorVec = BoundedVec<wrt_error::Error, 256, RuntimeProvider>;
