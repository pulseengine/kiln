use std::boxed::Box;

pub use wrt_error::{Error, ErrorCategory};
pub use wrt_foundation::{Result, resource::ResourceRepresentation};

// Comment: BlockType, FuncType, RefType, ValueType now require MemoryProvider
// parameters
