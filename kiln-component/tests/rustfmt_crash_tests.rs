use std::boxed::Box;

pub use kiln_error::{Error, ErrorCategory};
pub use kiln_foundation::{Result, resource::ResourceRepresentation};

// Comment: BlockType, FuncType, RefType, ValueType now require MemoryProvider
// parameters
