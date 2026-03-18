//! Type aliases for kiln-runtime

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
/// Value stack type
pub type ValueStackVec = Vec<kiln_foundation::Value>;

/// Call stack type
pub type CallStackVec = Vec<crate::core_types::CallFrame>;

/// Local variables vector type
pub type LocalsVec = Vec<kiln_foundation::Value>;

/// Global variables vector type
pub type GlobalsVec = Vec<crate::global::Global>;

/// Functions vector type
pub type FunctionsVec = Vec<crate::func::Function>;

/// Imports vector type
pub type ImportsVec<T> = Vec<T>;

/// Exports vector type
pub type ExportsVec<T> = Vec<T>;

/// Tables vector type
pub type TablesVec = Vec<crate::table::Table>;

/// Memories vector type
pub type MemoriesVec = Vec<crate::memory::Memory>;

/// Element segments vector type
pub type ElementsVec = Vec<kiln_foundation::types::ElementSegment>;

/// Data segments vector type
pub type DataVec = Vec<kiln_foundation::types::DataSegment>;

// Instruction vectors
/// Instructions vector type
// Instructions module is temporarily disabled in kiln-decoder
// pub type InstructionVec = Vec<kiln_decoder::instructions::Instruction>;
pub type InstructionVec = Vec<crate::prelude::Instruction>;

/// Branch targets vector type
pub type BranchTargetsVec = Vec<u32>;

// Module instance vectors
/// Module instances vector type
pub type ModuleInstanceVec = Vec<crate::module_instance::ModuleInstance>;

/// Function bodies vector type
pub type FunctionBodiesVec = Vec<Vec<u8>>;

// Memory and table data
/// Memory data vector type
pub type MemoryDataVec = Vec<u8>;

/// Table data vector type
pub type TableDataVec = Vec<Option<crate::prelude::RefValue>>;

// String type for runtime
/// Runtime string type
pub type RuntimeString = String;

// Maps for runtime state
/// Function map type
pub type FunctionMap = HashMap<u32, crate::func::Function>;

/// Global map type
pub type GlobalMap = HashMap<u32, crate::global::Global>;

/// Memory map type
pub type MemoryMap = HashMap<u32, crate::memory::Memory>;

/// Table map type
pub type TableMap = HashMap<u32, crate::table::Table>;

// CFI and instrumentation types
/// CFI checks vector type
pub type CfiCheckVec = Vec<crate::cfi_engine::CfiCheck>;

/// Instrumentation points vector type
pub type InstrumentationVec = Vec<crate::execution::InstrumentationPoint>;

// Generic byte vector for raw data
/// Byte vector type
pub type ByteVec = Vec<u8>;

// Error collection for batch operations
/// Error vector type
pub type ErrorVec = Vec<kiln_error::Error>;
