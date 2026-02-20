//! Bounded Infrastructure for Instructions
//!
//! This module provides bounded alternatives for instruction collections
//! to ensure static memory allocation throughout instruction handling.

use wrt_foundation::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    safe_memory::NoStdProvider,
};

/// Budget-aware memory provider for instructions (32KB)
pub type InstructionProvider = NoStdProvider<32768>;

/// Maximum number of instructions in a function
pub const MAX_INSTRUCTIONS_PER_FUNCTION: usize = 8192;

/// Maximum number of branch table targets
pub const MAX_BR_TABLE_TARGETS: usize = 1024;

/// Maximum number of basic blocks in a function
pub const MAX_BASIC_BLOCKS: usize = 1024;

/// Maximum number of local variables
pub const MAX_LOCAL_VARIABLES: usize = 512;

/// Maximum stack depth for validation
pub const MAX_STACK_DEPTH: usize = 1024;

/// Maximum control frame depth
pub const MAX_CONTROL_FRAMES: usize = 128;

/// Maximum number of edges in control flow graph
pub const MAX_CFG_EDGES: usize = 2048;

/// Maximum label length
pub const MAX_LABEL_LENGTH: usize = 64;

/// Bounded vector for instructions
pub type BoundedInstructionVec<T> =
    BoundedVec<T, MAX_INSTRUCTIONS_PER_FUNCTION, InstructionProvider>;

/// Bounded vector for branch table targets
pub type BoundedBrTableTargets = BoundedVec<u32, MAX_BR_TABLE_TARGETS, InstructionProvider>;

/// Bounded vector for basic blocks
pub type BoundedBasicBlockVec<T> = BoundedVec<T, MAX_BASIC_BLOCKS, InstructionProvider>;

/// Bounded vector for local variables
pub type BoundedLocalVec<T> = BoundedVec<T, MAX_LOCAL_VARIABLES, InstructionProvider>;

/// Bounded vector for stack validation
pub type BoundedStackVec<T> = BoundedVec<T, MAX_STACK_DEPTH, InstructionProvider>;

/// Bounded vector for control frames
pub type BoundedControlFrameVec<T> = BoundedVec<T, MAX_CONTROL_FRAMES, InstructionProvider>;

/// Bounded vector for CFG edges
pub type BoundedCfgEdgeVec<T> = BoundedVec<T, MAX_CFG_EDGES, InstructionProvider>;

/// Bounded string for labels
pub type BoundedLabelString = BoundedString<MAX_LABEL_LENGTH>;

/// Create a new bounded instruction vector
///
/// # Errors
///
/// Returns an error if memory allocation fails
pub fn new_instruction_vec<T>() -> wrt_error::Result<BoundedInstructionVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = InstructionProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded branch table targets vector
///
/// # Errors
///
/// Returns an error if memory allocation fails
pub fn new_br_table_targets() -> wrt_error::Result<BoundedBrTableTargets> {
    let provider = InstructionProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded basic block vector
///
/// # Errors
///
/// Returns an error if memory allocation fails
pub fn new_basic_block_vec<T>() -> wrt_error::Result<BoundedBasicBlockVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = InstructionProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded local variable vector
///
/// # Errors
///
/// Returns an error if memory allocation fails
pub fn new_local_vec<T>() -> wrt_error::Result<BoundedLocalVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = InstructionProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded stack vector
///
/// # Errors
///
/// Returns an error if memory allocation fails
pub fn new_stack_vec<T>() -> wrt_error::Result<BoundedStackVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = InstructionProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded control frame vector
///
/// # Errors
///
/// Returns an error if memory allocation fails
pub fn new_control_frame_vec<T>() -> wrt_error::Result<BoundedControlFrameVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = InstructionProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded CFG edge vector
///
/// # Errors
///
/// Returns an error if memory allocation fails
pub fn new_cfg_edge_vec<T>() -> wrt_error::Result<BoundedCfgEdgeVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = InstructionProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded label string
///
/// # Errors
///
/// Returns an error if string creation fails
pub fn new_label_string() -> wrt_error::Result<BoundedLabelString> {
    Ok(BoundedString::from_str_truncate("")?)
}

/// Create a bounded label string from str
///
/// # Errors
///
/// Returns an error if string conversion fails
pub fn bounded_label_from_str(s: &str) -> wrt_error::Result<BoundedLabelString> {
    Ok(BoundedString::try_from_str(s)?)
}
