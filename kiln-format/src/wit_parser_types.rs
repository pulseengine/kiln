//! Type definitions for WIT (WebAssembly Interface Types) parser
//! 
//! This module contains only the type definitions, without trait implementations,
//! to avoid circular dependencies.

#[cfg(feature = "std")]
use std::boxed::Box;
#[cfg(all(not(feature = "std")))]
use std::boxed::Box;

use wrt_foundation::{
    BoundedVec, BoundedString,
    bounded::MAX_GENERATIVE_TYPES,
    MemoryProvider,
};

/// Type constructors for WIT parser types

/// Bounded string for WIT identifiers and names (64 bytes max)
pub type WitBoundedString<P> = BoundedString<64, P>;
/// Small bounded string for WIT parameters and short names (32 bytes max)
pub type WitBoundedStringSmall<P> = BoundedString<32, P>;
/// Large bounded string for WIT error messages and long strings (128 bytes max)
pub type WitBoundedStringLarge<P> = BoundedString<128, P>;

/// A WIT world definition containing imports, exports, and type definitions
#[derive(Debug, Clone, PartialEq)]
pub struct WitWorld<P: MemoryProvider> {
    /// World name
    pub name: WitBoundedString<P>,
    /// Imported items
    pub imports: BoundedVec<WitImport<P>, MAX_GENERATIVE_TYPES, P>,
    /// Exported items
    pub exports: BoundedVec<WitExport<P>, MAX_GENERATIVE_TYPES, P>,
    /// Type definitions
    pub types: BoundedVec<WitTypeDef<P>, MAX_GENERATIVE_TYPES, P>,
}

/// A WIT interface definition containing functions and types
#[derive(Debug, Clone, PartialEq)]
pub struct WitInterface<P: MemoryProvider> {
    /// Interface name
    pub name: WitBoundedString<P>,
    /// Functions in this interface
    pub functions: BoundedVec<WitFunction<P>, MAX_GENERATIVE_TYPES, P>,
    /// Type definitions in this interface
    pub types: BoundedVec<WitTypeDef<P>, MAX_GENERATIVE_TYPES, P>,
}

/// A WIT import statement
#[derive(Debug, Clone, PartialEq)]
pub struct WitImport<P: MemoryProvider> {
    /// Import name
    pub name: WitBoundedString<P>,
    /// Imported item
    pub item: WitItem<P>,
}

/// A WIT export statement
#[derive(Debug, Clone, PartialEq)]
pub struct WitExport<P: MemoryProvider> {
    /// Export name
    pub name: WitBoundedString<P>,
    /// Exported item
    pub item: WitItem<P>,
}

/// A WIT item that can be imported or exported
#[derive(Debug, Clone, PartialEq)]
pub enum WitItem<P: MemoryProvider> {
    /// Function item
    Function(WitFunction<P>),
    /// Interface item
    Interface(WitInterface<P>),
    /// Type item
    Type(WitType<P>),
    /// Instance item
    Instance(WitInstance<P>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitFunction<P: MemoryProvider> {
    pub name: WitBoundedString<P>,
    pub params: BoundedVec<WitParam<P>, 32, P>,
    pub results: BoundedVec<WitResult<P>, 16, P>,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitParam<P: MemoryProvider> {
    pub name: WitBoundedStringSmall<P>,
    pub ty: WitType<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitResult<P: MemoryProvider> {
    pub name: Option<WitBoundedStringSmall<P>>,
    pub ty: WitType<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitInstance<P: MemoryProvider> {
    pub interface_name: WitBoundedString<P>,
    pub args: BoundedVec<WitInstanceArg<P>, 32, P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitInstanceArg<P: MemoryProvider> {
    pub name: WitBoundedStringSmall<P>,
    pub value: WitValue<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WitValue<P: MemoryProvider> {
    Type(WitType<P>),
    Instance(WitBoundedString<P>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitTypeDef<P: MemoryProvider> {
    pub name: WitBoundedString<P>,
    pub ty: WitType<P>,
    pub is_resource: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WitType<P: MemoryProvider> {
    /// Basic primitive types
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
    String,
    
    /// Compound types
    List(Box<WitType<P>>),
    Option(Box<WitType<P>>),
    Result {
        ok: Option<Box<WitType<P>>>,
        err: Option<Box<WitType<P>>>,
    },
    Tuple(BoundedVec<WitType<P>, 16, P>),
    Record(WitRecord<P>),
    Variant(WitVariant<P>),
    Enum(WitEnum<P>),
    Flags(WitFlags<P>),
    
    /// Resource types
    Own(WitBoundedString<P>),
    Borrow(WitBoundedString<P>),
    
    /// Named type reference
    Named(WitBoundedString<P>),
    
    /// Stream and Future for async support
    Stream(Box<WitType<P>>),
    Future(Box<WitType<P>>),
}

/// A WIT record type with named fields
#[derive(Debug, Clone, PartialEq)]
pub struct WitRecord<P: MemoryProvider> {
    /// The fields of the record
    pub fields: BoundedVec<WitRecordField<P>, 32, P>,
}

/// A field in a WIT record
#[derive(Debug, Clone, PartialEq)]
pub struct WitRecordField<P: MemoryProvider> {
    /// The name of the field
    pub name: WitBoundedStringSmall<P>,
    /// The type of the field
    pub ty: WitType<P>,
}

/// A WIT variant type with multiple cases
#[derive(Debug, Clone, PartialEq)]
pub struct WitVariant<P: MemoryProvider> {
    /// The cases of the variant
    pub cases: BoundedVec<WitVariantCase<P>, 32, P>,
}

/// A case in a WIT variant
#[derive(Debug, Clone, PartialEq)]
pub struct WitVariantCase<P: MemoryProvider> {
    /// The name of the case
    pub name: WitBoundedStringSmall<P>,
    /// The optional payload type
    pub ty: Option<WitType<P>>,
}

/// A WIT enum type with named cases
#[derive(Debug, Clone, PartialEq)]
pub struct WitEnum<P: MemoryProvider> {
    /// The cases of the enum
    pub cases: BoundedVec<WitBoundedStringSmall<P>, 32, P>,
}

/// A WIT flags type with named flags
#[derive(Debug, Clone, PartialEq)]
pub struct WitFlags<P: MemoryProvider> {
    /// The flags
    pub flags: BoundedVec<WitBoundedStringSmall<P>, 32, P>,
}

/// Parsed representation of a WIT document
#[derive(Debug, Clone, PartialEq)]
pub struct WitDocument<P: MemoryProvider> {
    /// The worlds defined in this document
    pub worlds: BoundedVec<WitWorld<P>, 16, P>,
    /// The interfaces defined in this document
    pub interfaces: BoundedVec<WitInterface<P>, 32, P>,
}

/// Error types for WIT parsing
#[derive(Debug, Clone, PartialEq)]
pub enum WitParseError<P: MemoryProvider> {
    /// Unexpected end of input
    UnexpectedEnd,
    /// Invalid syntax encountered
    InvalidSyntax(WitBoundedStringLarge<P>),
    /// Unknown type referenced
    UnknownType(WitBoundedString<P>),
    /// Too many items for bounded collections
    TooManyItems,
    /// Invalid identifier format
    InvalidIdentifier(WitBoundedString<P>),
    /// Duplicate definition found
    DuplicateDefinition(WitBoundedString<P>),
}

impl<P: MemoryProvider> core::fmt::Display for WitParseError<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WitParseError::UnexpectedEnd => write!(f, "Unexpected end of input"),
            WitParseError::InvalidSyntax(msg) => write!(f, "Invalid syntax: {}", msg.as_str().unwrap_or("<error>")),
            WitParseError::UnknownType(name) => write!(f, "Unknown type: {}", name.as_str().unwrap_or("<unknown>")),
            WitParseError::TooManyItems => write!(f, "Too many items"),
            WitParseError::InvalidIdentifier(name) => write!(f, "Invalid identifier: {}", name.as_str().unwrap_or("<invalid>")),
            WitParseError::DuplicateDefinition(name) => write!(f, "Duplicate definition: {}", name.as_str().unwrap_or("<duplicate>")),
        }
    }
}

#[cfg(feature = "std")]
impl<P: MemoryProvider> std::error::Error for WitParseError<P> {}