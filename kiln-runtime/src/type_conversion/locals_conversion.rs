//! Type conversion utilities for local variables
//!
//! This module provides conversion between different representations of local
//! variables used in the WRT execution pipeline.

// alloc is imported in lib.rs with proper feature gates
#[cfg(any(feature = "std", feature = "alloc"))]
use alloc::vec::Vec;

use wrt_error::Result;
use wrt_foundation::{
    bounded::BoundedVec,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    types::{
        LocalEntry,
        ValueType,
    },
    MemoryProvider,
};

use crate::bounded_runtime_infra::{
    create_runtime_provider,
    RuntimeProvider,
};

/// Convert a flat Vec<ValueType> to a BoundedVec<LocalEntry> by grouping
/// consecutive types of the same kind into LocalEntry structs with count and
/// type.
pub fn convert_locals_to_bounded_with_provider(
    locals: &[ValueType],
    provider: RuntimeProvider,
) -> Result<BoundedVec<LocalEntry, 256, RuntimeProvider>> {

    let mut bounded_locals = BoundedVec::new(provider)?;

    if locals.is_empty() {
        return Ok(bounded_locals);
    }

    // Group consecutive locals of the same type
    let mut current_type = locals[0];
    let mut current_count = 1u32;

    for &local_type in &locals[1..] {
        if local_type == current_type {
            current_count += 1;
        } else {
            // Add the accumulated group
            bounded_locals.push(LocalEntry {
                count:      current_count,
                value_type: current_type,
            })?;

            // Start new group
            current_type = local_type;
            current_count = 1;
        }
    }

    // Add the final group
    bounded_locals.push(LocalEntry {
        count:      current_count,
        value_type: current_type,
    })?;

    Ok(bounded_locals)
}

/// Convert a flat Vec<ValueType> to a BoundedVec<LocalEntry> by grouping
/// consecutive types of the same kind into LocalEntry structs with count and
/// type.
/// 
/// This is a backward-compatible wrapper that creates its own provider.
pub fn convert_locals_to_bounded(
    locals: &[ValueType],
) -> Result<BoundedVec<LocalEntry, 256, RuntimeProvider>> {
    let provider = create_runtime_provider()?;
    convert_locals_to_bounded_with_provider(locals, provider)
}

/// Convert a BoundedVec<LocalEntry> back to a flat Vec<ValueType>
/// This is useful for compatibility with APIs that expect the flat
/// representation
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn expand_locals_to_flat(
    bounded_locals: &BoundedVec<LocalEntry, 256, RuntimeProvider>,
) -> Result<Vec<ValueType>> {
    let mut flat_locals = Vec::new();

    for local_entry in bounded_locals.iter() {
        for _ in 0..local_entry.count {
            flat_locals.push(local_entry.value_type);
        }
    }

    Ok(flat_locals)
}

