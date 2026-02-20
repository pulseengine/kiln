//! Simple decoder types using StaticVec (no Provider)
//!
//! These are simplified, self-contained versions of wrt-foundation types
//! that use the new StaticVec collections instead of Provider-based BoundedVec.
//!
//! This allows wrt-decoder to migrate incrementally without waiting for
//! full wrt-foundation migration.

use crate::bounded_decoder_infra::{MAX_FUNCTION_PARAMS, MAX_FUNCTION_RESULTS, MAX_NAME_LENGTH};
use wrt_format::types::ValueType;
use wrt_foundation::collections::StaticVec;

/// Function type with static inline storage
#[derive(Debug, Clone)]
pub struct SimpleFuncType {
    pub params: StaticVec<ValueType, MAX_FUNCTION_PARAMS>,
    pub results: StaticVec<ValueType, MAX_FUNCTION_RESULTS>,
}

impl SimpleFuncType {
    /// Create a new function type
    pub fn new() -> Self {
        Self {
            params: StaticVec::new(),
            results: StaticVec::new(),
        }
    }

    /// Create from iterators
    pub fn from_iters(
        params_iter: impl IntoIterator<Item = ValueType>,
        results_iter: impl IntoIterator<Item = ValueType>,
    ) -> wrt_error::Result<Self> {
        let mut func_type = Self::new();
        for param in params_iter {
            func_type.params.push(param)?;
        }
        for result in results_iter {
            func_type.results.push(result)?;
        }
        Ok(func_type)
    }
}

impl Default for SimpleFuncType {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple string wrapper using StaticVec
pub type SimpleString = StaticVec<u8, MAX_NAME_LENGTH>;

/// Create a simple string from &str
pub fn simple_string_from_str(s: &str) -> wrt_error::Result<SimpleString> {
    let mut string = StaticVec::new();
    for byte in s.bytes() {
        string.push(byte)?;
    }
    Ok(string)
}
