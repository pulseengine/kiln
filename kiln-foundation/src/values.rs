// Kiln - kiln-foundation
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly value representations
//!
//! This module provides datatypes for representing WebAssembly values at
//! runtime.

// Always need alloc for Component Model types
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use core::fmt;
#[cfg(feature = "std")]
use std::fmt;

// GC heap reference types require shared mutable access (Arc<Mutex<>>)
// to implement WebAssembly GC reference semantics where multiple locals/stack
// entries can refer to the same struct/array heap object.
// Uses Arc<Mutex> for thread safety (Send + Sync).
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};
#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use kiln_sync::Mutex;

// Core imports
use kiln_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

// Publicly re-export FloatBits32 and FloatBits64 from the local float_repr module
pub use crate::float_repr::{
    FloatBits32,
    FloatBits64,
};
// // use std::format; // Removed: format! should come from prelude
use crate::traits::LittleEndian as TraitLittleEndian; // Alias trait
// Use the canonical LittleEndian trait and BytesWriter from crate::traits
use crate::traits::{
    BoundedCapacity,
    BytesWriter,
    Checksummable,
    DefaultMemoryProvider,
    FromBytes,
    LittleEndian,
    ReadStream,
    ToBytes,
    WriteStream,
};
use crate::types::{
    ValueType,
    MAX_ARRAY_ELEMENTS,
    MAX_STRUCT_FIELDS,
}; // Import ValueType and RefType
use crate::{
    bounded::BoundedVec,
    prelude::{
        Debug,
        Eq,
        PartialEq,
    },
    verification::Checksum,
    MemoryProvider,
}; // Added for Checksummable

/// GC-managed struct reference for WebAssembly 3.0
#[derive(Debug, Clone, PartialEq, Eq, core::hash::Hash)]
pub struct StructRef<
    P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq = DefaultMemoryProvider,
> {
    /// Type index of the struct
    pub type_index: u32,
    /// Field values
    pub fields:     BoundedVec<Value, MAX_STRUCT_FIELDS, P>,
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> StructRef<P> {
    /// Create a new struct reference
    pub fn new(type_index: u32, provider: P) -> kiln_error::Result<Self> {
        let fields = BoundedVec::new(provider).map_err(Error::from)?;
        Ok(Self { type_index, fields })
    }

    /// Set a field value
    pub fn set_field(&mut self, index: usize, value: Value) -> kiln_error::Result<()> {
        if index < self.fields.len() {
            self.fields.set(index, value).map_err(Error::from).map(|_| ())
        } else {
            Err(Error::validation_error("Field index out of bounds"))
        }
    }

    /// Get a field value
    pub fn get_field(&self, index: usize) -> kiln_error::Result<Value> {
        self.fields.get(index).map_err(Error::from)
    }

    /// Add a field value
    pub fn add_field(&mut self, value: Value) -> kiln_error::Result<()> {
        self.fields.push(value).map_err(Error::from)
    }
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> Default
    for StructRef<P>
{
    fn default() -> Self {
        let provider = P::default();
        Self::new(0, provider).expect("Default StructRef creation failed")
    }
}

/// GC-managed array reference for WebAssembly 3.0
#[derive(Debug, Clone, PartialEq, Eq, core::hash::Hash)]
pub struct ArrayRef<
    P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq = DefaultMemoryProvider,
> {
    /// Type index of the array
    pub type_index: u32,
    /// Array elements
    pub elements:   BoundedVec<Value, MAX_ARRAY_ELEMENTS, P>,
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> ArrayRef<P> {
    /// Create a new array reference
    pub fn new(type_index: u32, provider: P) -> kiln_error::Result<Self> {
        let elements = BoundedVec::new(provider).map_err(Error::from)?;
        Ok(Self {
            type_index,
            elements,
        })
    }

    /// Create an array with initial size and value
    pub fn with_size(
        type_index: u32,
        size: usize,
        init_value: Value,
        provider: P,
    ) -> kiln_error::Result<Self> {
        let mut elements = BoundedVec::new(provider).map_err(Error::from)?;
        for _ in 0..size {
            elements.push(init_value.clone()).map_err(Error::from)?;
        }
        Ok(Self {
            type_index,
            elements,
        })
    }

    /// Get array length
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if array is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Get element at index
    pub fn get(&self, index: usize) -> kiln_error::Result<Value> {
        self.elements.get(index).map_err(Error::from)
    }

    /// Set element at index
    pub fn set(&mut self, index: usize, value: Value) -> kiln_error::Result<()> {
        if index < self.elements.len() {
            self.elements.set(index, value).map_err(Error::from).map(|_| ())
        } else {
            Err(Error::validation_error("Array index out of bounds"))
        }
    }

    /// Push element to array
    pub fn push(&mut self, value: Value) -> kiln_error::Result<()> {
        self.elements.push(value).map_err(Error::from)
    }
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> Default
    for ArrayRef<P>
{
    fn default() -> Self {
        let provider = P::default();
        Self::new(0, provider).expect("Default ArrayRef creation failed")
    }
}

/// GC heap-allocated struct reference with shared ownership.
///
/// Wraps `Arc<Mutex<StructRef>>` to provide WebAssembly GC reference semantics:
/// multiple locals and stack entries can refer to the same struct, and mutations
/// through one reference are visible through all others.
/// Uses Arc<Mutex> for thread safety (Send + Sync).
#[derive(Clone)]
pub struct GcStructRef(pub Arc<Mutex<StructRef<DefaultMemoryProvider>>>);

impl GcStructRef {
    /// Create a new GC struct reference wrapping the given StructRef
    pub fn new(inner: StructRef<DefaultMemoryProvider>) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    /// Create a sentinel value used in WAST test comparison.
    pub fn sentinel() -> Self {
        Self::new(StructRef::new(u32::MAX, DefaultMemoryProvider::default())
            .unwrap_or_else(|_| StructRef { type_index: u32::MAX, fields: Default::default() }))
    }

    /// Check if this is a sentinel (used for "any non-null" matching).
    pub fn is_sentinel(&self) -> bool {
        self.type_index() == u32::MAX
    }

    /// Identity comparison: true if both point to the same allocation
    pub fn arc_ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Acquire the lock (abstracts over std vs no_std Mutex API differences)
    #[cfg(feature = "std")]
    fn lock_inner(&self) -> std::sync::MutexGuard<'_, StructRef<DefaultMemoryProvider>> {
        self.0.lock().expect("GcStructRef lock poisoned")
    }

    /// Acquire the lock (no_std version - kiln_sync::Mutex::lock returns guard directly)
    #[cfg(not(feature = "std"))]
    fn lock_inner(&self) -> kiln_sync::KilnMutexGuard<'_, StructRef<DefaultMemoryProvider>> {
        self.0.lock()
    }

    /// Get the type index of the struct
    pub fn type_index(&self) -> u32 {
        self.lock_inner().type_index
    }

    /// Get a field value
    pub fn get_field(&self, index: usize) -> kiln_error::Result<Value> {
        self.lock_inner().get_field(index)
    }

    /// Set a field value
    pub fn set_field(&self, index: usize, value: Value) -> kiln_error::Result<()> {
        self.lock_inner().set_field(index, value)
    }

    /// Add a field value
    pub fn add_field(&self, value: Value) -> kiln_error::Result<()> {
        self.lock_inner().add_field(value)
    }
}

impl fmt::Debug for GcStructRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GcStructRef(type={})", self.type_index())
    }
}

impl PartialEq for GcStructRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for GcStructRef {}

impl core::hash::Hash for GcStructRef {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

/// GC heap-allocated array reference with shared ownership.
///
/// Wraps `Arc<Mutex<ArrayRef>>` to provide WebAssembly GC reference semantics:
/// multiple locals and stack entries can refer to the same array, and mutations
/// through one reference are visible through all others.
/// Uses Arc<Mutex> for thread safety (Send + Sync).
#[derive(Clone)]
pub struct GcArrayRef(pub Arc<Mutex<ArrayRef<DefaultMemoryProvider>>>);

impl GcArrayRef {
    /// Create a new GC array reference wrapping the given ArrayRef
    pub fn new(inner: ArrayRef<DefaultMemoryProvider>) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    /// Create a sentinel value used in WAST test comparison.
    /// Matches any non-null array reference.
    pub fn sentinel() -> Self {
        Self::new(ArrayRef::new(u32::MAX, DefaultMemoryProvider::default())
            .unwrap_or_else(|_| ArrayRef { type_index: u32::MAX, elements: Default::default() }))
    }

    /// Check if this is a sentinel (used for "any non-null" matching).
    pub fn is_sentinel(&self) -> bool {
        self.type_index() == u32::MAX
    }

    /// Identity comparison: true if both point to the same allocation
    pub fn arc_ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Acquire the lock (abstracts over std vs no_std Mutex API differences)
    #[cfg(feature = "std")]
    fn lock_inner(&self) -> std::sync::MutexGuard<'_, ArrayRef<DefaultMemoryProvider>> {
        self.0.lock().expect("GcArrayRef lock poisoned")
    }

    /// Acquire the lock (no_std version)
    #[cfg(not(feature = "std"))]
    fn lock_inner(&self) -> kiln_sync::KilnMutexGuard<'_, ArrayRef<DefaultMemoryProvider>> {
        self.0.lock()
    }

    /// Get the type index of the array
    pub fn type_index(&self) -> u32 {
        self.lock_inner().type_index
    }

    /// Get array length
    pub fn len(&self) -> usize {
        self.lock_inner().len()
    }

    /// Check if array is empty
    pub fn is_empty(&self) -> bool {
        self.lock_inner().is_empty()
    }

    /// Get element at index
    pub fn get(&self, index: usize) -> kiln_error::Result<Value> {
        self.lock_inner().get(index)
    }

    /// Set element at index
    pub fn set(&self, index: usize, value: Value) -> kiln_error::Result<()> {
        self.lock_inner().set(index, value)
    }

    /// Push element to array
    pub fn push(&self, value: Value) -> kiln_error::Result<()> {
        self.lock_inner().push(value)
    }
}

impl fmt::Debug for GcArrayRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GcArrayRef(type={}, len={})", self.type_index(), self.len())
    }
}

impl PartialEq for GcArrayRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for GcArrayRef {}

impl core::hash::Hash for GcArrayRef {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

impl Checksummable for GcStructRef {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update(self.type_index() as u8);
    }
}

impl Checksummable for GcArrayRef {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update(self.type_index() as u8);
    }
}

/// Represents a WebAssembly runtime value
#[derive(Debug, Clone, core::hash::Hash)]
#[allow(clippy::derived_hash_with_manual_eq)]
pub enum Value {
    /// 32-bit integer
    I32(i32),
    /// 64-bit integer
    I64(i64),
    /// 32-bit float
    F32(FloatBits32),
    /// 64-bit float
    F64(FloatBits64),
    /// 128-bit vector
    V128(V128),
    /// Function reference
    FuncRef(Option<FuncRef>),
    /// External reference
    ExternRef(Option<ExternRef>),
    /// Generic opaque reference (often an index), serialized as a u32/i32.
    Ref(u32),
    /// 16-bit vector (represented internally as V128)
    I16x8(V128),
    /// Struct reference (WebAssembly 3.0 GC)
    /// Uses GcStructRef for shared heap storage (Rc<RefCell<>> semantics)
    StructRef(Option<GcStructRef>),
    /// Array reference (WebAssembly 3.0 GC)
    /// Uses GcArrayRef for shared heap storage (Rc<RefCell<>> semantics)
    ArrayRef(Option<GcArrayRef>),
    /// Exception reference (Exception Handling proposal)
    ExnRef(Option<u32>),
    /// i31 reference (WebAssembly 3.0 GC) - unboxed 31-bit integer
    /// The value is stored as a 32-bit signed integer but only 31 bits are used.
    /// None represents a null i31ref.
    I31Ref(Option<i32>),
    /// Component Model extensions
    Bool(bool),
    S8(i8),
    U8(u8),
    S16(i16),
    U16(u16),
    S32(i32),
    U32(u32),
    S64(i64),
    U64(u64),
    Char(char),
    String(alloc::string::String),
    List(alloc::vec::Vec<Value>),
    Tuple(alloc::vec::Vec<Value>),
    Record(alloc::vec::Vec<(alloc::string::String, Value)>),
    Variant(alloc::string::String, Option<alloc::boxed::Box<Value>>),
    Enum(alloc::string::String),
    Option(Option<alloc::boxed::Box<Value>>),
    Result(core::result::Result<alloc::boxed::Box<Value>, alloc::boxed::Box<Value>>),
    Flags(alloc::vec::Vec<alloc::string::String>),
    Own(u32),
    Borrow(u32),
    Void,
    /// Stream handle (Component Model async)
    /// The u32 represents a handle ID that can be used with async operations
    Stream(u32),
    /// Future handle (Component Model async)
    /// The u32 represents a handle ID that can be used with async operations
    Future(u32),
}

// Manual PartialEq implementation for Value
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::I32(a), Value::I32(b)) => a == b,
            (Value::I64(a), Value::I64(b)) => a == b,
            // Handle NaN comparison for floats: NaN != NaN
            (Value::F32(a), Value::F32(b)) => {
                (a.value().is_nan() && b.value().is_nan()) || (a.value() == b.value())
            },
            (Value::F64(a), Value::F64(b)) => {
                (a.value().is_nan() && b.value().is_nan()) || (a.value() == b.value())
            },
            (Value::V128(a), Value::V128(b)) => a == b,
            (Value::FuncRef(a), Value::FuncRef(b)) => a == b,
            (Value::ExternRef(a), Value::ExternRef(b)) => a == b,
            (Value::ExnRef(a), Value::ExnRef(b)) => a == b,
            (Value::I31Ref(a), Value::I31Ref(b)) => a == b,
            (Value::Ref(a), Value::Ref(b)) => a == b,
            (Value::I16x8(a), Value::I16x8(b)) => a == b,
            (Value::StructRef(a), Value::StructRef(b)) => a == b,
            (Value::ArrayRef(a), Value::ArrayRef(b)) => a == b,
            (Value::Stream(a), Value::Stream(b)) => a == b,
            (Value::Future(a), Value::Future(b)) => a == b,
            _ => false, // Different types are not equal
        }
    }
}

impl Eq for Value {}

impl Default for Value {
    fn default() -> Self {
        // Return FuncRef(None) as default because:
        // 1. Tables store Option<Value> and commonly use FuncRef values
        // 2. Option<T>::serialized_size() uses T::default().serialized_size()
        // 3. FuncRef has size 6 (1 disc + 1 flag + 4 padding), larger than I32's size 5
        // 4. This ensures BoundedVec slots are large enough for all reference types
        Value::FuncRef(None)
    }
}

/// A WebAssembly v128 value used for SIMD operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct V128 {
    /// The 128-bit value represented as 16 bytes
    pub bytes: [u8; 16],
}

impl V128 {
    /// Create a new v128 value from 16 bytes
    #[must_use]
    pub fn new(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }

    /// Create a v128 filled with zeros
    #[must_use]
    pub const fn zero() -> Self {
        Self { bytes: [0; 16] }
    }
}

impl AsRef<[u8]> for V128 {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

// Create a helper function for creating a v128 value
/// Helper function to create a new V128 value
#[must_use]
pub fn v128(bytes: [u8; 16]) -> V128 {
    V128::new(bytes)
}

/// Function reference type
///
/// In multi-instance scenarios (e.g., Component Model shared tables),
/// `instance_id` identifies which instance's function space this reference
/// belongs to. When `None`, the reference is resolved in the current instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, core::hash::Hash)]
pub struct FuncRef {
    /// Function index within the instance's function space
    pub index: u32,
    /// Optional instance ID for cross-instance references (shared tables)
    /// When None, resolved in the current instance's function space.
    /// When Some(id), resolved in instance `id`'s function space.
    pub instance_id: Option<u32>,
}

impl FuncRef {
    /// Creates a new `FuncRef` from an index (local to current instance)
    #[must_use]
    pub fn from_index(index: u32) -> Self {
        Self { index, instance_id: None }
    }

    /// Creates a new `FuncRef` with a specific instance ID (cross-instance)
    #[must_use]
    pub fn from_index_with_instance(index: u32, instance_id: u32) -> Self {
        Self { index, instance_id: Some(instance_id) }
    }
}

/// External reference type
#[derive(Debug, Clone, Copy, PartialEq, Eq, core::hash::Hash)]
pub struct ExternRef {
    /// Reference index
    pub index: u32,
}

impl Value {
    /// Creates a default value for the given WebAssembly value type.
    ///
    /// This function returns a zero value for numeric types and None for
    /// reference types.
    #[must_use]
    pub const fn default_for_type(ty: &ValueType) -> Self {
        match ty {
            ValueType::I32 => Value::I32(0),
            ValueType::I64 => Value::I64(0),
            ValueType::F32 => Value::F32(FloatBits32(0)),
            ValueType::F64 => Value::F64(FloatBits64(0)),
            ValueType::V128 => Value::V128(V128::zero()),
            ValueType::I16x8 => Value::I16x8(V128::zero()),
            ValueType::FuncRef => Value::FuncRef(None),
            ValueType::NullFuncRef => Value::FuncRef(None), // Bottom type defaults to null
            ValueType::ExternRef => Value::ExternRef(None),
            ValueType::StructRef(_) => Value::StructRef(None),
            ValueType::ArrayRef(_) => Value::ArrayRef(None),
            ValueType::ExnRef => Value::ExnRef(None),
            ValueType::I31Ref => Value::I31Ref(None),
            ValueType::AnyRef => Value::ExternRef(None), // AnyRef uses externref representation
            ValueType::EqRef => Value::I31Ref(None),     // EqRef defaults to i31ref
            ValueType::TypedFuncRef(_, _) => Value::FuncRef(None), // Typed funcref defaults to null
            ValueType::NoneRef => Value::StructRef(None),     // Bottom of any hierarchy
            ValueType::NoExternRef => Value::ExternRef(None), // Bottom of extern hierarchy
            ValueType::NoExnRef => Value::ExnRef(None),       // Bottom of exn hierarchy
        }
    }

    /// Returns the value type of this `Value`.
    #[must_use]
    pub fn value_type(&self) -> ValueType {
        match self {
            Self::I32(_) => ValueType::I32,
            Self::I64(_) => ValueType::I64,
            Self::F32(_) => ValueType::F32,
            Self::F64(_) => ValueType::F64,
            Self::V128(_) => ValueType::V128,
            Self::I16x8(_) => ValueType::I16x8,
            Self::FuncRef(_) => ValueType::FuncRef,
            Self::ExternRef(_) => ValueType::ExternRef,
            Self::ExnRef(_) => ValueType::ExnRef,
            Self::Ref(_) => ValueType::I32,
            Self::StructRef(Some(s)) => ValueType::StructRef(s.type_index()),
            Self::StructRef(None) => ValueType::StructRef(0), // Default type index for null
            Self::ArrayRef(Some(a)) => ValueType::ArrayRef(a.type_index()),
            Self::ArrayRef(None) => ValueType::ArrayRef(0), // Default type index for null
            Self::I31Ref(_) => ValueType::I31Ref,
            // Component Model types - these are not standard WebAssembly types
            // Stream/Future handles are represented as I32 in the canonical ABI
            Self::Bool(_) | Self::S8(_) | Self::U8(_) | Self::S16(_) | Self::U16(_) |
            Self::S32(_) | Self::U32(_) | Self::S64(_) | Self::U64(_) | Self::Char(_) |
            Self::String(_) | Self::List(_) | Self::Tuple(_) | Self::Record(_) |
            Self::Variant(_, _) | Self::Enum(_) | Self::Option(_) | Self::Result(_) |
            Self::Flags(_) | Self::Own(_) | Self::Borrow(_) | Self::Void |
            Self::Stream(_) | Self::Future(_) => ValueType::I32,
        }
    }

    /// Checks if the value matches the provided type.
    #[must_use]
    pub fn matches_type(&self, ty: &ValueType) -> bool {
        match (self, ty) {
            (Self::I32(_), ValueType::I32) => true,
            (Self::I64(_), ValueType::I64) => true,
            (Self::F32(_), ValueType::F32) => true,
            (Self::F64(_), ValueType::F64) => true,
            (Self::V128(_), ValueType::V128) => true,
            (Self::I16x8(_), ValueType::I16x8) => true,
            (Self::FuncRef(_), ValueType::FuncRef) => true,
            (Self::ExternRef(_), ValueType::ExternRef) => true,
            (Self::ExnRef(_), ValueType::ExnRef) => true,
            (Self::Ref(_), ValueType::I32) => true,
            (Self::StructRef(Some(s)), ValueType::StructRef(idx)) => s.type_index() == *idx,
            (Self::StructRef(None), ValueType::StructRef(_)) => true, // Null matches any struct type
            (Self::ArrayRef(Some(a)), ValueType::ArrayRef(idx)) => a.type_index() == *idx,
            (Self::ArrayRef(None), ValueType::ArrayRef(_)) => true, // Null matches any array type
            // GC reference types
            (Self::I31Ref(_), ValueType::I31Ref) => true,
            (Self::I31Ref(_), ValueType::EqRef) => true,   // i31ref <: eqref
            (Self::I31Ref(_), ValueType::AnyRef) => true,   // i31ref <: anyref
            // TypedFuncRef is used for any concrete typed reference (ref null? $t)
            // Match with the appropriate Value variant based on the value's actual type
            (Self::FuncRef(_), ValueType::TypedFuncRef(_, _)) => true,
            (Self::ArrayRef(Some(a)), ValueType::TypedFuncRef(idx, _)) => a.type_index() == *idx,
            (Self::ArrayRef(None), ValueType::TypedFuncRef(_, _)) => true,
            (Self::StructRef(Some(s)), ValueType::TypedFuncRef(idx, _)) => s.type_index() == *idx,
            (Self::StructRef(None), ValueType::TypedFuncRef(_, _)) => true,
            // Subtyping: struct/array refs are subtypes of anyref/eqref
            (Self::StructRef(_), ValueType::AnyRef) => true,
            (Self::StructRef(_), ValueType::EqRef) => true,
            (Self::ArrayRef(_), ValueType::AnyRef) => true,
            (Self::ArrayRef(_), ValueType::EqRef) => true,
            _ => false,
        }
    }

    /// Returns the underlying value as a `u32` if it's an `i32`.
    #[must_use]
    pub const fn as_u32(&self) -> Option<u32> {
        match *self {
            Value::I32(val) => Some(val as u32),
            _ => None,
        }
    }

    /// Returns the underlying value as an `i32` if it's an `i32`.
    #[must_use]
    pub const fn as_i32(&self) -> Option<i32> {
        match *self {
            Value::I32(val) => Some(val),
            _ => None,
        }
    }

    /// Tries to convert the `Value` into an `i32`.
    /// Returns an error if the value is not an `I32`.
    pub fn into_i32(self) -> kiln_error::Result<i32> {
        match self {
            Value::I32(v) => Ok(v),
            _ => Err(Error::type_error("Value is not an i32")),
        }
    }

    /// Attempts to extract an i64 value if this Value is an I64.
    #[must_use]
    pub const fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(v) => Some(*v),
            _ => None,
        }
    }

    /// Attempts to extract an f32 value if this Value is an F32.
    #[must_use]
    pub const fn as_f32(&self) -> Option<f32> {
        match self {
            Self::F32(v) => Some(v.value()),
            _ => None,
        }
    }

    /// Attempts to extract an f64 value if this Value is an F64.
    #[must_use]
    pub const fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(v) => Some(v.value()),
            _ => None,
        }
    }

    /// Attempts to extract a `FuncRef` index if this Value is a `FuncRef`.
    pub fn as_func_ref(&self) -> Option<Option<u32>> {
        match self {
            Self::FuncRef(fr) => Some(fr.as_ref().map(|r| r.index)),
            _ => None,
        }
    }

    /// Attempts to extract an `ExternRef` index if this Value is an
    /// `ExternRef`.
    pub fn as_extern_ref(&self) -> Option<Option<u32>> {
        match self {
            Self::ExternRef(er) => Some(er.as_ref().map(|r| r.index)),
            _ => None,
        }
    }

    /// Returns the underlying `u32` if this `Value` is a `Ref`.
    #[must_use]
    pub const fn as_ref_u32(&self) -> Option<u32> {
        match self {
            Self::Ref(val) => Some(*val),
            _ => None,
        }
    }

    /// Attempts to interpret this `Value` as a boolean (`false` if zero, `true`
    /// otherwise). Only applicable to integer types `I32` and `I64`.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::I32(v) => Some(*v != 0),
            Self::I64(v) => Some(*v != 0),
            _ => None,
        }
    }

    /// Attempts to extract an i8 value if this `Value` is an `I32`.
    #[must_use]
    pub const fn as_i8(&self) -> Option<i8> {
        match self {
            Self::I32(v) => Some(*v as i8),
            _ => None,
        }
    }

    /// Attempts to extract a u8 value if this `Value` is an `I32`.
    #[must_use]
    pub const fn as_u8(&self) -> Option<u8> {
        match self {
            Self::I32(v) => Some(*v as u8),
            _ => None,
        }
    }

    /// Attempts to extract an i16 value if this `Value` is an `I32`.
    #[must_use]
    pub const fn as_i16(&self) -> Option<i16> {
        match self {
            Self::I32(v) => Some(*v as i16),
            _ => None,
        }
    }

    /// Attempts to extract a u16 value if this `Value` is an `I32`.
    #[must_use]
    pub const fn as_u16(&self) -> Option<u16> {
        match self {
            Self::I32(v) => Some(*v as u16),
            _ => None,
        }
    }

    /// Attempts to extract a u64 value if this `Value` is a `U64`.
    #[must_use]
    pub const fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U64(v) => Some(*v),
            _ => None,
        }
    }

    /// Attempts to extract a char value if this `Value` is a `Char`.
    #[must_use]
    pub const fn as_char(&self) -> Option<char> {
        match self {
            Self::Char(c) => Some(*c),
            _ => None,
        }
    }

    /// Attempts to extract a string slice if this `Value` is a `String`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Attempts to extract a list reference if this `Value` is a `List`.
    pub fn as_list(&self) -> Option<&alloc::vec::Vec<Value>> {
        match self {
            Self::List(list) => Some(list),
            _ => None,
        }
    }

    /// Attempts to extract a tuple reference if this `Value` is a `Tuple`.
    pub fn as_tuple(&self) -> Option<&alloc::vec::Vec<Value>> {
        match self {
            Self::Tuple(tuple) => Some(tuple),
            _ => None,
        }
    }

    /// Attempts to extract a record reference if this `Value` is a `Record`.
    pub fn as_record(&self) -> Option<&alloc::vec::Vec<(alloc::string::String, Value)>> {
        match self {
            Self::Record(record) => Some(record),
            _ => None,
        }
    }

    /// Attempts to extract variant data if this `Value` is a `Variant`.
    pub fn as_variant(&self) -> Option<(&str, Option<&Value>)> {
        match self {
            Self::Variant(name, val) => {
                Some((name.as_str(), val.as_ref().map(|b| b.as_ref())))
            },
            _ => None,
        }
    }

    /// Attempts to extract flags reference if this `Value` is `Flags`.
    pub fn as_flags(&self) -> Option<&alloc::vec::Vec<alloc::string::String>> {
        match self {
            Self::Flags(flags) => Some(flags),
            _ => None,
        }
    }

    /// Attempts to extract the bytes of a V128 value.
    pub fn as_v128(&self) -> kiln_error::Result<[u8; 16]> {
        match self {
            Self::V128(v) => Ok(v.bytes),
            Self::I16x8(v) => Ok(v.bytes), // I16x8 is also V128 internally
            _ => Err(Error::runtime_execution_error(
                "Value is not a V128 or I16x8 type",
            )),
        }
    }

    /// Efficiently copies the value for simple (Copy-like) variants.
    ///
    /// For WebAssembly core types (I32, I64, F32, F64, V128, FuncRef, ExternRef, Ref, I16x8),
    /// this performs a direct copy without heap allocation. For other variants
    /// (String, List, etc.), it falls back to clone().
    ///
    /// This is an optimization for LocalGet/LocalTee operations where the common
    /// case is accessing numeric locals which are Copy types internally.
    #[must_use]
    #[inline]
    pub fn copy_value(&self) -> Self {
        match self {
            // Core WebAssembly numeric types - all Copy
            Self::I32(v) => Self::I32(*v),
            Self::I64(v) => Self::I64(*v),
            Self::F32(v) => Self::F32(*v),
            Self::F64(v) => Self::F64(*v),
            Self::V128(v) => Self::V128(*v),
            Self::I16x8(v) => Self::I16x8(*v),
            // Reference types - now Copy
            Self::FuncRef(v) => Self::FuncRef(*v),
            Self::ExternRef(v) => Self::ExternRef(*v),
            Self::ExnRef(v) => Self::ExnRef(*v),
            Self::I31Ref(v) => Self::I31Ref(*v),
            Self::Ref(v) => Self::Ref(*v),
            // Simple Copy types from Component Model
            Self::Bool(v) => Self::Bool(*v),
            Self::S8(v) => Self::S8(*v),
            Self::U8(v) => Self::U8(*v),
            Self::S16(v) => Self::S16(*v),
            Self::U16(v) => Self::U16(*v),
            Self::S32(v) => Self::S32(*v),
            Self::U32(v) => Self::U32(*v),
            Self::S64(v) => Self::S64(*v),
            Self::U64(v) => Self::U64(*v),
            Self::Char(v) => Self::Char(*v),
            Self::Own(v) => Self::Own(*v),
            Self::Borrow(v) => Self::Borrow(*v),
            Self::Void => Self::Void,
            // Stream/Future handles are Copy (u32)
            Self::Stream(v) => Self::Stream(*v),
            Self::Future(v) => Self::Future(*v),
            // GC types and complex types - fall back to clone
            Self::StructRef(_) | Self::ArrayRef(_) |
            Self::String(_) | Self::List(_) | Self::Tuple(_) | Self::Record(_) |
            Self::Variant(_, _) | Self::Enum(_) | Self::Option(_) | Self::Result(_) |
            Self::Flags(_) => self.clone(),
        }
    }

    /// Tries to convert the `Value` into an `i32` after truncating from `f32`.
    /// Returns an error if the value is not an `F32` or if truncation fails.
    pub fn into_i32_from_f32(self) -> kiln_error::Result<i32> {
        match self {
            Value::F32(f_val) => {
                let f = f_val.value();
                if f.is_nan() || f.is_infinite() {
                    Err(Error::type_error("Invalid f32 to i32 conversion (NaN/Inf)"))
                } else if f < (i32::MIN as f32) || f > (i32::MAX as f32) {
                    Err(Error::type_error(
                        "Invalid f32 to i32 conversion (overflow)",
                    ))
                } else {
                    Ok(f as i32)
                }
            },
            _ => Err(Error::type_error("Value is not an f32 for i32 conversion")),
        }
    }

    /// Tries to convert the `Value` into an `i64` after truncating from `f64`.
    /// Returns an error if the value is not an `F64` or if truncation fails.
    pub fn into_i64_from_f64(self) -> kiln_error::Result<i64> {
        match self {
            Value::F64(f_val) => {
                let f = f_val.value();
                if f.is_nan() || f.is_infinite() {
                    Err(Error::type_error("Invalid f64 to i64 conversion (NaN/Inf)"))
                } else if f < (i64::MIN as f64) || f > (i64::MAX as f64) {
                    Err(Error::type_error(
                        "Invalid f64 to i64 conversion (overflow)",
                    ))
                } else {
                    Ok(f as i64)
                }
            },
            _ => Err(Error::type_error("Value is not an f64 for i64 conversion")),
        }
    }

    /// Creates a `FuncRef` value.
    #[must_use]
    pub fn func_ref(func_idx: Option<u32>) -> Self {
        match func_idx {
            Some(idx) => Value::FuncRef(Some(FuncRef::from_index(idx))),
            None => Value::FuncRef(None),
        }
    }

    /// Writes the `Value` to the given writer in little-endian format.
    pub fn write_le_bytes<W: BytesWriter>(&self, writer: &mut W) -> kiln_error::Result<()> {
        match self {
            Value::I32(val) => writer.write_all(&val.to_le_bytes()),
            Value::I64(val) => writer.write_all(&val.to_le_bytes()),
            Value::F32(val) => writer.write_all(&val.0.to_le_bytes()),
            Value::F64(val) => writer.write_all(&val.0.to_le_bytes()),
            Value::V128(val) | Value::I16x8(val) => writer.write_all(&val.bytes),
            Value::FuncRef(Some(fr)) => writer.write_all(&fr.index.to_le_bytes()),
            Value::ExternRef(Some(er)) => writer.write_all(&er.index.to_le_bytes()),
            Value::Ref(r) => writer.write_all(&r.to_le_bytes()),
            Value::FuncRef(None) | Value::ExternRef(None) | Value::ExnRef(None) => {
                // Null reference, often represented as a specific integer pattern (e.g., all
                // ones or zero) For now, let's serialize as 0, assuming it
                // represents null. This needs to align with deserialization and
                // runtime expectations.
                writer.write_all(&0u32.to_le_bytes())
            },
            Value::ExnRef(Some(idx)) => writer.write_all(&idx.to_le_bytes()),
            Value::I31Ref(Some(v)) => writer.write_all(&v.to_le_bytes()),
            Value::I31Ref(None) => writer.write_all(&0i32.to_le_bytes()),
            Value::StructRef(Some(s)) => writer.write_all(&s.type_index().to_le_bytes()),
            Value::StructRef(None) => writer.write_all(&0u32.to_le_bytes()),
            Value::ArrayRef(Some(a)) => writer.write_all(&a.type_index().to_le_bytes()),
            Value::ArrayRef(None) => writer.write_all(&0u32.to_le_bytes()),
            // Component Model types - simplified serialization as i32
            Value::Bool(b) => writer.write_all(&(*b as i32).to_le_bytes()),
            Value::S8(v) => writer.write_all(&(*v as i32).to_le_bytes()),
            Value::U8(v) => writer.write_all(&(*v as i32).to_le_bytes()),
            Value::S16(v) => writer.write_all(&(*v as i32).to_le_bytes()),
            Value::U16(v) => writer.write_all(&(*v as i32).to_le_bytes()),
            Value::S32(v) => writer.write_all(&v.to_le_bytes()),
            Value::U32(v) => writer.write_all(&v.to_le_bytes()),
            Value::S64(v) => writer.write_all(&v.to_le_bytes()),
            Value::U64(v) => writer.write_all(&v.to_le_bytes()),
            Value::Char(c) => writer.write_all(&(*c as u32).to_le_bytes()),
            Value::String(_) | Value::List(_) | Value::Tuple(_) | Value::Record(_) |
            Value::Variant(_, _) | Value::Enum(_) | Value::Option(_) | Value::Result(_) |
            Value::Flags(_) | Value::Void => {
                // For complex types, write as 0 (handle would go here in full implementation)
                writer.write_all(&0u32.to_le_bytes())
            },
            Value::Own(v) | Value::Borrow(v) | Value::Stream(v) | Value::Future(v) => {
                // Write handle value
                writer.write_all(&v.to_le_bytes())
            },
        }
    }

    /// Reads a `Value` from the given byte slice in little-endian format.
    pub fn from_le_bytes(bytes: &[u8], ty: &ValueType) -> kiln_error::Result<Self> {
        match ty {
            ValueType::I32 => {
                if bytes.len() < 4 {
                    return Err(Error::parse_error("Insufficient bytes for I32"));
                }
                Ok(Value::I32(i32::from_le_bytes(
                    bytes[0..4].try_into().map_err(|_| {
                        Error::runtime_execution_error("Failed to convert bytes to i32")
                    })?,
                )))
            },
            ValueType::I64 => {
                if bytes.len() < 8 {
                    return Err(Error::parse_error("Insufficient bytes for I64"));
                }
                Ok(Value::I64(i64::from_le_bytes(
                    bytes[0..8].try_into().map_err(|_| {
                        Error::runtime_execution_error("Failed to convert bytes to i64")
                    })?,
                )))
            },
            ValueType::F32 => {
                if bytes.len() < 4 {
                    return Err(Error::parse_error("Insufficient bytes for F32"));
                }
                Ok(Value::F32(FloatBits32(u32::from_le_bytes(
                    bytes[0..4].try_into().map_err(|_| {
                        Error::runtime_execution_error("Failed to convert bytes to f32")
                    })?,
                ))))
            },
            ValueType::F64 => {
                if bytes.len() < 8 {
                    return Err(Error::parse_error("Insufficient bytes for F64"));
                }
                Ok(Value::F64(FloatBits64(u64::from_le_bytes(
                    bytes[0..8].try_into().map_err(|_| {
                        Error::runtime_execution_error("Failed to convert bytes to f64")
                    })?,
                ))))
            },
            ValueType::V128 | ValueType::I16x8 => {
                if bytes.len() < 16 {
                    return Err(Error::parse_error("Insufficient bytes for V128/I16x8"));
                }
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&bytes[0..16]);
                if *ty == ValueType::V128 {
                    Ok(Value::V128(V128 { bytes: arr }))
                } else {
                    Ok(Value::I16x8(V128 { bytes: arr }))
                }
            },
            ValueType::FuncRef => {
                if bytes.len() < 4 {
                    return Err(Error::parse_error("Insufficient bytes for FuncRef"));
                }
                let idx = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
                    Error::runtime_execution_error("Failed to convert bytes to FuncRef index")
                })?);
                // Assuming 0 or a specific pattern might mean None, for now, always Some.
                // The interpretation of the index (e.g. if 0 means null) is context-dependent.
                Ok(Value::FuncRef(Some(FuncRef::from_index(idx))))
            },
            ValueType::NullFuncRef => {
                // Bottom type - always null
                Ok(Value::FuncRef(None))
            },
            ValueType::ExternRef => {
                if bytes.len() < 4 {
                    return Err(Error::parse_error("Insufficient bytes for ExternRef"));
                }
                let idx = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
                    Error::runtime_execution_error("Failed to convert bytes to ExternRef index")
                })?);
                Ok(Value::ExternRef(Some(ExternRef { index: idx })))
            },
            ValueType::StructRef(_) => {
                // For aggregate types, we don't support direct byte deserialization yet
                // These require more complex GC-aware deserialization
                Ok(Value::StructRef(None))
            },
            ValueType::ArrayRef(_) => {
                // For aggregate types, we don't support direct byte deserialization yet
                // These require more complex GC-aware deserialization
                Ok(Value::ArrayRef(None))
            },
            ValueType::ExnRef => {
                // Exception references not yet supported for byte deserialization
                Ok(Value::ExnRef(None))
            },
            ValueType::I31Ref => {
                if bytes.len() < 4 {
                    return Err(Error::parse_error("Insufficient bytes for I31Ref"));
                }
                let val = i32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
                    Error::runtime_execution_error("Failed to convert bytes to I31Ref")
                })?);
                // Zero is interpreted as null
                if val == 0 {
                    Ok(Value::I31Ref(None))
                } else {
                    Ok(Value::I31Ref(Some(val)))
                }
            },
            ValueType::AnyRef => {
                // AnyRef uses externref representation for now
                Ok(Value::ExternRef(None))
            },
            ValueType::EqRef => {
                // EqRef defaults to null i31ref
                Ok(Value::I31Ref(None))
            },
            ValueType::TypedFuncRef(_, _) => {
                // Typed function references not yet supported for byte deserialization
                Ok(Value::FuncRef(None))
            },
            ValueType::NoneRef => Ok(Value::StructRef(None)),     // Bottom of any hierarchy
            ValueType::NoExternRef => Ok(Value::ExternRef(None)), // Bottom of extern hierarchy
            ValueType::NoExnRef => Ok(Value::ExnRef(None)),       // Bottom of exn hierarchy
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::I32(v) => write!(f, "i32:{v}"),
            Value::I64(v) => write!(f, "i64:{v}"),
            Value::F32(v) => write!(f, "f32:{}", v.value()),
            Value::F64(v) => write!(f, "f64:{}", v.value()),
            Value::V128(v) => write!(f, "v128:{v:?}"),
            Value::FuncRef(Some(v)) => write!(f, "funcref:{}", v.index),
            Value::FuncRef(None) => write!(f, "funcref:null"),
            Value::ExternRef(Some(v)) => write!(f, "externref:{}", v.index),
            Value::ExternRef(None) => write!(f, "externref:null"),
            Value::ExnRef(Some(v)) => write!(f, "exnref:{v}"),
            Value::ExnRef(None) => write!(f, "exnref:null"),
            Value::I31Ref(Some(v)) => write!(f, "i31ref:{v}"),
            Value::I31Ref(None) => write!(f, "i31ref:null"),
            Value::Ref(v) => write!(f, "ref:{v}"),
            Value::I16x8(v) => write!(f, "i16x8:{v:?}"),
            Value::StructRef(Some(v)) => write!(f, "structref:type{}", v.type_index()),
            Value::StructRef(None) => write!(f, "structref:null"),
            Value::ArrayRef(Some(v)) => write!(f, "arrayref:type{}[{}]", v.type_index(), v.len()),
            Value::ArrayRef(None) => write!(f, "arrayref:null"),
            // Component Model types
            Value::Bool(b) => write!(f, "bool:{b}"),
            Value::S8(v) => write!(f, "s8:{v}"),
            Value::U8(v) => write!(f, "u8:{v}"),
            Value::S16(v) => write!(f, "s16:{v}"),
            Value::U16(v) => write!(f, "u16:{v}"),
            Value::S32(v) => write!(f, "s32:{v}"),
            Value::U32(v) => write!(f, "u32:{v}"),
            Value::S64(v) => write!(f, "s64:{v}"),
            Value::U64(v) => write!(f, "u64:{v}"),
            Value::Char(c) => write!(f, "char:{c}"),
            Value::String(s) => write!(f, "string:{s}"),
            Value::List(items) => write!(f, "list[{}]", items.len()),
            Value::Tuple(items) => write!(f, "tuple[{}]", items.len()),
            Value::Record(fields) => write!(f, "record[{}]", fields.len()),
            Value::Variant(name, val) => match val {
                Some(_) => write!(f, "variant:{name}(...)"),
                None => write!(f, "variant:{name}"),
            },
            Value::Enum(name) => write!(f, "enum:{name}"),
            Value::Option(val) => match val {
                Some(_) => write!(f, "option:Some(...)"),
                None => write!(f, "option:None"),
            },
            Value::Result(res) => match res {
                Ok(_) => write!(f, "result:Ok(...)"),
                Err(_) => write!(f, "result:Err(...)"),
            },
            Value::Flags(flags) => write!(f, "flags[{}]", flags.len()),
            Value::Own(h) => write!(f, "own:{h}"),
            Value::Borrow(h) => write!(f, "borrow:{h}"),
            Value::Void => write!(f, "void"),
            Value::Stream(h) => write!(f, "stream:{h}"),
            Value::Future(h) => write!(f, "future:{h}"),
        }
    }
}

/// `AsRef<[u8]>` implementation for Value
///
/// This implementation allows a Value to be treated as a byte slice
/// reference. It is primarily used for memory operations.
impl AsRef<[u8]> for Value {
    fn as_ref(&self) -> &[u8] {
        // This implementation is problematic as Value doesn't have a direct, simple
        // byte representation. It should likely be removed or rethought. For
        // now, returning an empty slice to satisfy a potential trait bound
        // elsewhere, but this needs review. panic!("Value::as_ref<[u8]> is not
        // meaningfully implemented";
        &[] // Placeholder, likely incorrect for general use
    }
}

// Implement LittleEndian for V128 here as V128 is defined in this module.
impl LittleEndian for V128 {
    fn from_le_bytes(bytes: &[u8]) -> kiln_error::Result<Self> {
        if bytes.len() != 16 {
            return Err(Error::runtime_execution_error(
                "V128 requires exactly 16 bytes",
            ));
        }
        let arr: [u8; 16] = bytes.try_into().map_err(|_| {
            Error::new(
                ErrorCategory::System,
                codes::CONVERSION_ERROR,
                "Failed to convert slice to V128 byte array",
            )
        })?;
        Ok(V128 { bytes: arr })
    }

    fn write_le_bytes<W: BytesWriter>(&self, writer: &mut W) -> kiln_error::Result<()> {
        writer.write_all(&self.bytes)
    }
}

impl Checksummable for V128 {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update_slice(&self.bytes);
    }
}

impl ToBytes for V128 {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream, // Provider not typically needed for simple types
    ) -> kiln_error::Result<()> {
        // Write the bytes directly to the stream
        writer.write_all(&self.bytes)
    }
    // to_bytes method is provided by the trait with DefaultMemoryProvider
}

impl FromBytes for V128 {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream, // Provider not typically needed for simple types
    ) -> kiln_error::Result<Self> {
        // Read exactly 16 bytes for V128
        let mut arr = [0u8; 16];
        reader.read_exact(&mut arr)?;
        Ok(V128 { bytes: arr })
    }
    // from_bytes method is provided by the trait with DefaultMemoryProvider
}

impl Checksummable for FuncRef {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.index.update_checksum(checksum);
        // Encode instance_id as u32 (0 = None, N+1 = Some(N))
        let instance_id_encoded = match self.instance_id {
            None => 0u32,
            Some(id) => id.wrapping_add(1),
        };
        instance_id_encoded.update_checksum(checksum);
    }
}

impl ToBytes for FuncRef {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<()> {
        self.index.to_bytes_with_provider(writer, provider)?;
        // Encode instance_id: 0 = None, N+1 = Some(N)
        let instance_id_encoded: u32 = match self.instance_id {
            None => 0,
            Some(id) => id.wrapping_add(1),
        };
        instance_id_encoded.to_bytes_with_provider(writer, provider)
    }
    // to_bytes method is provided by the trait with DefaultMemoryProvider
}

impl FromBytes for FuncRef {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<Self> {
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let instance_id_encoded = u32::from_bytes_with_provider(reader, provider)?;
        let instance_id = if instance_id_encoded == 0 {
            None
        } else {
            Some(instance_id_encoded.wrapping_sub(1))
        };
        Ok(FuncRef { index, instance_id })
    }
    // from_bytes method is provided by the trait with DefaultMemoryProvider
}

impl Checksummable for ExternRef {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.index.update_checksum(checksum);
    }
}

impl ToBytes for ExternRef {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<()> {
        // Delegate to the u32 implementation
        self.index.to_bytes_with_provider(writer, provider)
    }
    // to_bytes method is provided by the trait with DefaultMemoryProvider
}

impl FromBytes for ExternRef {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<Self> {
        // Delegate to the u32 implementation
        let index = u32::from_bytes_with_provider(reader, provider)?;
        Ok(ExternRef { index })
    }
    // from_bytes method is provided by the trait with DefaultMemoryProvider
}

impl Checksummable for Value {
    fn update_checksum(&self, checksum: &mut Checksum) {
        let discriminant_byte = match self {
            Value::I32(_) => 0u8,
            Value::I64(_) => 1u8,
            Value::F32(_) => 2u8,
            Value::F64(_) => 3u8,
            Value::V128(_) => 4u8,
            Value::FuncRef(_) => 5u8,
            Value::ExternRef(_) => 6u8,
            Value::ExnRef(_) => 35u8,   // Exception reference
            Value::I31Ref(_) => 36u8,   // i31 reference (GC)
            Value::Ref(_) => 7u8,       // Generic Ref
            Value::I16x8(_) => 8u8,     // I16x8, distinct from V128 for checksum
            Value::StructRef(_) => 9u8, // Struct reference
            Value::ArrayRef(_) => 10u8, // Array reference
            // Component Model types
            Value::Bool(_) => 11u8,
            Value::S8(_) => 12u8,
            Value::U8(_) => 13u8,
            Value::S16(_) => 14u8,
            Value::U16(_) => 15u8,
            Value::S32(_) => 16u8,
            Value::U32(_) => 17u8,
            Value::S64(_) => 18u8,
            Value::U64(_) => 19u8,
            Value::Char(_) => 20u8,
            Value::String(_) => 21u8,
            Value::List(_) => 22u8,
            Value::Tuple(_) => 23u8,
            Value::Record(_) => 24u8,
            Value::Variant(_, _) => 25u8,
            Value::Enum(_) => 26u8,
            Value::Option(_) => 27u8,
            Value::Result(_) => 28u8,
            Value::Flags(_) => 29u8,
            Value::Own(_) => 30u8,
            Value::Borrow(_) => 31u8,
            Value::Void => 32u8,
            Value::Stream(_) => 33u8,
            Value::Future(_) => 34u8,
        };
        checksum.update(discriminant_byte);

        match self {
            Value::I32(v) => v.update_checksum(checksum),
            Value::I64(v) => v.update_checksum(checksum),
            Value::F32(v) => v.update_checksum(checksum),
            Value::F64(v) => v.update_checksum(checksum),
            Value::V128(v) | Value::I16x8(v) => v.update_checksum(checksum),
            Value::FuncRef(v) => v.update_checksum(checksum),
            Value::ExternRef(v) => v.update_checksum(checksum),
            Value::Ref(v) => v.update_checksum(checksum),
            Value::StructRef(v) => v.update_checksum(checksum),
            Value::ArrayRef(v) => v.update_checksum(checksum),
            // Component Model types - simplified checksum updates
            Value::Bool(v) => v.update_checksum(checksum),
            Value::S8(v) => v.update_checksum(checksum),
            Value::U8(v) => v.update_checksum(checksum),
            Value::S16(v) => v.update_checksum(checksum),
            Value::U16(v) => v.update_checksum(checksum),
            Value::S32(v) => v.update_checksum(checksum),
            Value::U32(v) => v.update_checksum(checksum),
            Value::S64(v) => v.update_checksum(checksum),
            Value::U64(v) => v.update_checksum(checksum),
            Value::Char(v) => (*v as u32).update_checksum(checksum),
            Value::String(s) => s.as_bytes().iter().for_each(|b| b.update_checksum(checksum)),
            Value::List(items) | Value::Tuple(items) => {
                items.len().update_checksum(checksum);
                items.iter().for_each(|item| item.update_checksum(checksum));
            },
            Value::Record(fields) => {
                fields.len().update_checksum(checksum);
                fields.iter().for_each(|(k, v)| {
                    k.as_bytes().iter().for_each(|b| b.update_checksum(checksum));
                    v.update_checksum(checksum);
                });
            },
            Value::Variant(name, val) => {
                name.as_bytes().iter().for_each(|b| b.update_checksum(checksum));
                if let Some(v) = val {
                    v.update_checksum(checksum);
                }
            },
            Value::Enum(name) => {
                name.as_bytes().iter().for_each(|b| b.update_checksum(checksum));
            },
            Value::Option(val) => {
                if let Some(v) = val {
                    v.update_checksum(checksum);
                }
            },
            Value::Result(res) => {
                match res {
                    Ok(v) | Err(v) => v.update_checksum(checksum),
                }
            },
            Value::Flags(flags) => {
                flags.len().update_checksum(checksum);
                flags.iter().for_each(|f| f.as_bytes().iter().for_each(|b| b.update_checksum(checksum)));
            },
            Value::Own(h) | Value::Borrow(h) | Value::Stream(h) | Value::Future(h) => {
                h.update_checksum(checksum);
            },
            Value::Void => {},
            Value::ExnRef(v) => v.update_checksum(checksum),
            Value::I31Ref(v) => v.update_checksum(checksum),
        }
    }
}

impl ToBytes for Value {
    fn serialized_size(&self) -> usize {
        // All reference types MUST return the same size for BoundedVec compatibility.
        // Tables store Option<Value> and the BoundedVec uses fixed-size slots based on
        // the default value's serialized_size. All ref variants that can appear in tables
        // must report the same size so set() doesn't overflow the allocated slot.
        //
        // The maximum ref type is FuncRef at 1+8=9 bytes. StructRef/ArrayRef serialize
        // just their type_index (4 bytes) in the table slot — the GC heap data lives in
        // the Arc, not in the serialized bytes.
        const REF_SERIALIZED_SIZE: usize = 1 + 8; // 1 flag + 8 data (matches FuncRef)

        // 1 byte for discriminant + variant-specific size
        1 + match self {
            Value::I32(_) => 4,
            Value::I64(_) => 8,
            Value::F32(_) => 4,
            Value::F64(_) => 8,
            Value::V128(_) | Value::I16x8(_) => 16,
            // All reference types use the same fixed size for BoundedVec table compatibility
            Value::FuncRef(_) => REF_SERIALIZED_SIZE,
            Value::ExternRef(_) => REF_SERIALIZED_SIZE,
            Value::ExnRef(_) => REF_SERIALIZED_SIZE,
            Value::I31Ref(_) => REF_SERIALIZED_SIZE,
            Value::Ref(_) => REF_SERIALIZED_SIZE,
            Value::StructRef(_) => REF_SERIALIZED_SIZE,
            Value::ArrayRef(_) => REF_SERIALIZED_SIZE,
            Value::Bool(_) => 1,
            Value::S8(_) | Value::U8(_) => 1,
            Value::S16(_) | Value::U16(_) => 2,
            Value::S32(_) | Value::U32(_) => 4,
            Value::S64(_) | Value::U64(_) => 8,
            Value::Char(_) => 4,
            // String: length (4) + content (variable, use a reasonable max)
            Value::String(s) => 4 + s.len(),
            // Complex types - use conservative estimate
            Value::List(_) | Value::Tuple(_) | Value::Record(_) => 64,
            Value::Variant(_, _) | Value::Enum(_) => 8,
            Value::Option(_) | Value::Result(_) => 16,
            Value::Flags(_) => 8,
            Value::Own(_) | Value::Borrow(_) | Value::Stream(_) | Value::Future(_) => 4,
            Value::Void => 0,
        }
    }

    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<()> {
        // Write discriminant byte
        let discriminant = match self {
            Value::I32(_) => 0u8,
            Value::I64(_) => 1u8,
            Value::F32(_) => 2u8,
            Value::F64(_) => 3u8,
            Value::V128(_) => 4u8,
            Value::FuncRef(_) => 5u8,
            Value::ExternRef(_) => 6u8,
            Value::Ref(_) => 7u8,       // Generic Ref, serialized as u32
            Value::I16x8(_) => 8u8,     // I16x8, serialized as V128
            Value::StructRef(_) => 9u8, // Struct reference
            Value::ArrayRef(_) => 10u8, // Array reference
            // Component Model types use same discriminants as checksum
            Value::Bool(_) => 11u8,
            Value::S8(_) => 12u8,
            Value::U8(_) => 13u8,
            Value::S16(_) => 14u8,
            Value::U16(_) => 15u8,
            Value::S32(_) => 16u8,
            Value::U32(_) => 17u8,
            Value::S64(_) => 18u8,
            Value::U64(_) => 19u8,
            Value::Char(_) => 20u8,
            Value::String(_) => 21u8,
            Value::List(_) => 22u8,
            Value::Tuple(_) => 23u8,
            Value::Record(_) => 24u8,
            Value::Variant(_, _) => 25u8,
            Value::Enum(_) => 26u8,
            Value::Option(_) => 27u8,
            Value::Result(_) => 28u8,
            Value::Flags(_) => 29u8,
            Value::Own(_) => 30u8,
            Value::Borrow(_) => 31u8,
            Value::Void => 32u8,
            Value::Stream(_) => 33u8,
            Value::Future(_) => 34u8,
            Value::ExnRef(_) => 35u8,
            Value::I31Ref(_) => 36u8,
        };
        writer.write_u8(discriminant)?;

        // Write the variant data
        match self {
            Value::I32(v) => v.to_bytes_with_provider(writer, provider)?,
            Value::I64(v) => v.to_bytes_with_provider(writer, provider)?,
            Value::F32(v) => v.to_bytes_with_provider(writer, provider)?,
            Value::F64(v) => v.to_bytes_with_provider(writer, provider)?,
            Value::V128(v) | Value::I16x8(v) => v.to_bytes_with_provider(writer, provider)?,
            Value::FuncRef(opt_v) => {
                // Write Some/None flag + always write 8 bytes for fixed size (4 index + 4 instance_id)
                writer.write_u8(if opt_v.is_some() { 1 } else { 0 })?;
                match opt_v {
                    Some(v) => v.to_bytes_with_provider(writer, provider)?,
                    None => {
                        writer.write_u32_le(0)?; // Padding for index
                        writer.write_u32_le(0)?; // Padding for instance_id
                    },
                }
            },
            Value::ExternRef(opt_v) => {
                // All ref types write exactly REF_SERIALIZED_SIZE (9) bytes
                writer.write_u8(if opt_v.is_some() { 1 } else { 0 })?;
                match opt_v {
                    Some(v) => { v.to_bytes_with_provider(writer, provider)?; writer.write_u32_le(0)?; },
                    None => { writer.write_u32_le(0)?; writer.write_u32_le(0)?; },
                }
            },
            Value::Ref(v) => {
                v.to_bytes_with_provider(writer, provider)?;
                // Pad to REF_SERIALIZED_SIZE
                writer.write_u32_le(0)?;
                writer.write_u8(0)?;
            },
            Value::StructRef(opt_v) => {
                // Write fixed-size: flag + type_index + padding (GC data lives in Arc heap)
                writer.write_u8(if opt_v.is_some() { 1 } else { 0 })?;
                match opt_v {
                    Some(v) => { writer.write_u32_le(v.type_index())?; writer.write_u32_le(0)?; },
                    None => { writer.write_u32_le(0)?; writer.write_u32_le(0)?; },
                }
            },
            Value::ArrayRef(opt_v) => {
                // Write fixed-size: flag + type_index + padding (GC data lives in Arc heap)
                writer.write_u8(if opt_v.is_some() { 1 } else { 0 })?;
                match opt_v {
                    Some(v) => { writer.write_u32_le(v.type_index())?; writer.write_u32_le(0)?; },
                    None => { writer.write_u32_le(0)?; writer.write_u32_le(0)?; },
                }
            },
            // Component Model types - simplified serialization
            Value::Bool(v) => writer.write_u8(if *v { 1 } else { 0 })?,
            Value::S8(v) => writer.write_i8(*v)?,
            Value::U8(v) => writer.write_u8(*v)?,
            Value::S16(v) => writer.write_i16_le(*v)?,
            Value::U16(v) => writer.write_u16_le(*v)?,
            Value::S32(v) => writer.write_i32_le(*v)?,
            Value::U32(v) => writer.write_u32_le(*v)?,
            Value::S64(v) => writer.write_i64_le(*v)?,
            Value::U64(v) => writer.write_u64_le(*v)?,
            Value::Char(v) => writer.write_u32_le(*v as u32)?,
            Value::String(_) | Value::List(_) | Value::Tuple(_) | Value::Record(_) |
            Value::Variant(_, _) | Value::Enum(_) | Value::Option(_) | Value::Result(_) |
            Value::Flags(_) => {
                // Complex types - not fully serializable in this simplified form
                writer.write_u32_le(0)?
            },
            Value::Own(h) | Value::Borrow(h) | Value::Stream(h) | Value::Future(h) => {
                writer.write_u32_le(*h)?;
            },
            Value::Void => {},
            Value::ExnRef(opt_v) => {
                // All ref types write exactly REF_SERIALIZED_SIZE (9) bytes
                writer.write_u8(if opt_v.is_some() { 1 } else { 0 })?;
                match opt_v {
                    Some(v) => { writer.write_u32_le(*v)?; writer.write_u32_le(0)?; },
                    None => { writer.write_u32_le(0)?; writer.write_u32_le(0)?; },
                }
            },
            Value::I31Ref(opt_v) => {
                // All ref types write exactly REF_SERIALIZED_SIZE (9) bytes
                writer.write_u8(if opt_v.is_some() { 1 } else { 0 })?;
                match opt_v {
                    Some(v) => { writer.write_i32_le(*v)?; writer.write_u32_le(0)?; },
                    None => { writer.write_i32_le(0)?; writer.write_u32_le(0)?; },
                }
            },
        }
        Ok(())
    }
}

impl FromBytes for Value {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<Self> {
        // Read discriminant byte
        let discriminant = reader.read_u8()?;

        // Parse the variant based on discriminant
        match discriminant {
            0 => {
                let v = i32::from_bytes_with_provider(reader, provider)?;
                Ok(Value::I32(v))
            },
            1 => {
                let v = i64::from_bytes_with_provider(reader, provider)?;
                Ok(Value::I64(v))
            },
            2 => {
                let v = FloatBits32::from_bytes_with_provider(reader, provider)?;
                Ok(Value::F32(v))
            },
            3 => {
                let v = FloatBits64::from_bytes_with_provider(reader, provider)?;
                Ok(Value::F64(v))
            },
            4 => {
                let v = V128::from_bytes_with_provider(reader, provider)?;
                Ok(Value::V128(v))
            },
            5 => {
                // FuncRef - always read 9 bytes (1 flag + 4 index + 4 instance_id) for fixed size
                let is_some = reader.read_u8()? == 1;
                if is_some {
                    let v = FuncRef::from_bytes_with_provider(reader, provider)?;
                    Ok(Value::FuncRef(Some(v)))
                } else {
                    let _ = reader.read_u32_le()?; // Skip index padding
                    let _ = reader.read_u32_le()?; // Skip instance_id padding
                    Ok(Value::FuncRef(None))
                }
            },
            6 => {
                // ExternRef - read 9 bytes (1 flag + 4 data + 4 padding) for fixed ref size
                let is_some = reader.read_u8()? == 1;
                if is_some {
                    let v = ExternRef::from_bytes_with_provider(reader, provider)?;
                    let _ = reader.read_u32_le()?; // Skip padding
                    Ok(Value::ExternRef(Some(v)))
                } else {
                    let _ = reader.read_u32_le()?; // Skip data
                    let _ = reader.read_u32_le()?; // Skip padding
                    Ok(Value::ExternRef(None))
                }
            },
            7 => {
                // Ref (u32) - read 9 bytes for fixed ref size
                let v = u32::from_bytes_with_provider(reader, provider)?;
                let _ = reader.read_u32_le()?; // Skip padding
                let _ = reader.read_u8()?; // Skip padding
                Ok(Value::Ref(v))
            },
            8 => {
                // I16x8 (V128)
                let v = V128::from_bytes_with_provider(reader, provider)?;
                Ok(Value::I16x8(v))
            },
            9 => {
                // StructRef - read 9 bytes (1 flag + 4 type_index + 4 padding)
                // Note: GC data lives in Arc heap, not serialized here.
                // On deserialization we create an empty StructRef with the type index.
                let is_some = reader.read_u8()? == 1;
                if is_some {
                    let type_index = reader.read_u32_le()?;
                    let _ = reader.read_u32_le()?; // Skip padding
                    let sref = StructRef::new(type_index, DefaultMemoryProvider::default())
                        .map_err(|_| Error::runtime_execution_error("Failed to create StructRef"))?;
                    Ok(Value::StructRef(Some(GcStructRef::new(sref))))
                } else {
                    let _ = reader.read_u32_le()?; // Skip data
                    let _ = reader.read_u32_le()?; // Skip padding
                    Ok(Value::StructRef(None))
                }
            },
            10 => {
                // ArrayRef - read 9 bytes (1 flag + 4 type_index + 4 padding)
                let is_some = reader.read_u8()? == 1;
                if is_some {
                    let type_index = reader.read_u32_le()?;
                    let _ = reader.read_u32_le()?; // Skip padding
                    let aref = ArrayRef::new(type_index, DefaultMemoryProvider::default())
                        .map_err(|_| Error::runtime_execution_error("Failed to create ArrayRef"))?;
                    Ok(Value::ArrayRef(Some(GcArrayRef::new(aref))))
                } else {
                    let _ = reader.read_u32_le()?; // Skip data
                    let _ = reader.read_u32_le()?; // Skip padding
                    Ok(Value::ArrayRef(None))
                }
            },
            // Component Model simple types (handles are u32)
            30 => {
                // Own handle
                let h = reader.read_u32_le()?;
                Ok(Value::Own(h))
            },
            31 => {
                // Borrow handle
                let h = reader.read_u32_le()?;
                Ok(Value::Borrow(h))
            },
            32 => {
                // Void
                Ok(Value::Void)
            },
            33 => {
                // Stream handle
                let h = reader.read_u32_le()?;
                Ok(Value::Stream(h))
            },
            34 => {
                // Future handle
                let h = reader.read_u32_le()?;
                Ok(Value::Future(h))
            },
            35 => {
                // ExnRef - read 9 bytes (1 flag + 4 data + 4 padding) for fixed ref size
                let flag = reader.read_u8()?;
                if flag != 0 {
                    let idx = reader.read_u32_le()?;
                    let _ = reader.read_u32_le()?; // Skip padding
                    Ok(Value::ExnRef(Some(idx)))
                } else {
                    let _ = reader.read_u32_le()?; // Skip data
                    let _ = reader.read_u32_le()?; // Skip padding
                    Ok(Value::ExnRef(None))
                }
            },
            36 => {
                // I31Ref - read 9 bytes (1 flag + 4 data + 4 padding) for fixed ref size
                let flag = reader.read_u8()?;
                if flag != 0 {
                    let v = reader.read_i32_le()?;
                    let _ = reader.read_u32_le()?; // Skip padding
                    Ok(Value::I31Ref(Some(v)))
                } else {
                    let _ = reader.read_i32_le()?; // Skip data
                    let _ = reader.read_u32_le()?; // Skip padding
                    Ok(Value::I31Ref(None))
                }
            },
            _ => Err(Error::runtime_execution_error(
                "Unknown discriminant byte in Value deserialization",
            )),
        }
    }
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> Checksummable
    for StructRef<P>
{
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.type_index.update_checksum(checksum);
        self.fields.update_checksum(checksum);
    }
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> ToBytes
    for StructRef<P>
{
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<()> {
        // Write type index
        self.type_index.to_bytes_with_provider(writer, provider)?;
        // Write field count
        writer.write_u32_le(self.fields.len() as u32)?;
        // Write fields
        for field in self.fields.iter() {
            field.to_bytes_with_provider(writer, provider)?;
        }
        Ok(())
    }
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> FromBytes
    for StructRef<P>
{
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<Self> {
        // Read type index
        let type_index = u32::from_bytes_with_provider(reader, provider)?;
        // Read field count
        let field_count = reader.read_u32_le()?;
        // Create struct with default provider
        let mut struct_ref = StructRef::new(type_index, P::default())?;
        // Read fields
        for _ in 0..field_count {
            let field = Value::from_bytes_with_provider(reader, provider)?;
            struct_ref.add_field(field)?;
        }
        Ok(struct_ref)
    }
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> Checksummable
    for ArrayRef<P>
{
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.type_index.update_checksum(checksum);
        self.elements.update_checksum(checksum);
    }
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> ToBytes
    for ArrayRef<P>
{
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<()> {
        // Write type index
        self.type_index.to_bytes_with_provider(writer, provider)?;
        // Write element count
        writer.write_u32_le(self.elements.len() as u32)?;
        // Write elements
        for element in self.elements.iter() {
            element.to_bytes_with_provider(writer, provider)?;
        }
        Ok(())
    }
}

impl<P: MemoryProvider + Default + Clone + core::fmt::Debug + PartialEq + Eq> FromBytes
    for ArrayRef<P>
{
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> kiln_error::Result<Self> {
        // Read type index
        let type_index = u32::from_bytes_with_provider(reader, provider)?;
        // Read element count
        let element_count = reader.read_u32_le()?;
        // Create array with default provider
        let mut array_ref = ArrayRef::new(type_index, P::default())?;
        // Read elements
        for _ in 0..element_count {
            let element = Value::from_bytes_with_provider(reader, provider)?;
            array_ref.push(element)?;
        }
        Ok(array_ref)
    }
}

