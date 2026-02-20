//! Temporary component model value representation
//!
//! This module provides a temporary implementation of component model values
//! until proper support is added to wrt-foundation.


/// Component model value representation
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Boolean value
    Bool(bool),
    /// Unsigned 8-bit integer
    U8(u8),
    /// Unsigned 32-bit integer
    U32(u32),
    /// Signed 32-bit integer
    S32(i32),
    /// Unsigned 64-bit integer
    U64(u64),
    /// String value
    String(String),
    /// List of values
    List(Vec<Value>),
    /// Record (struct) with named fields
    Record(Vec<(String, Value)>),
    /// Tuple of values
    Tuple(Vec<Value>),
    /// Optional value
    Option(Option<Box<Value>>),
}
