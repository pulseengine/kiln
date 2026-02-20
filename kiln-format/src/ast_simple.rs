//! Simplified AST types for WIT parsing
//!
//! This module provides basic AST node definitions that work with the current
//! wrt-foundation constraints while still providing source location tracking.

#[cfg(not(feature = "std"))]
use core::fmt;
#[cfg(feature = "std")]
use std::{boxed::Box, fmt, vec::Vec};
#[cfg(not(feature = "std"))]
use std::{boxed::Box, vec::Vec};

// Temporarily disabled - wit_parser module is commented out
// use crate::wit_parser::{WitBoundedString, WitBoundedStringSmall};
use wrt_foundation::{BoundedString, NoStdProvider};

// Type aliases to replace wit_parser types
type WitBoundedString = BoundedString<64>;
type WitBoundedStringSmall = BoundedString<32>;

/// Source location span for AST nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceSpan {
    /// Byte offset of the start of this span
    pub start: u32,
    /// Byte offset of the end of this span (exclusive)
    pub end: u32,
    /// Source file identifier
    pub file_id: u32,
}

impl SourceSpan {
    /// Create a new source span
    pub const fn new(start: u32, end: u32, file_id: u32) -> Self {
        Self {
            start,
            end,
            file_id,
        }
    }

    /// Create an empty span (used for synthetic nodes)
    pub const fn empty() -> Self {
        Self {
            start: 0,
            end: 0,
            file_id: 0,
        }
    }

    /// Get the length of the span in bytes
    pub const fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Check if the span is empty
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Merge two spans to create a span covering both
    pub fn merge(&self, other: &Self) -> Self {
        assert_eq!(
            self.file_id, other.file_id,
            "Cannot merge spans from different files"
        );
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            file_id: self.file_id,
        }
    }
}

/// An identifier with source location
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Identifier {
    /// The identifier text
    pub name: WitBoundedString,
    /// Source location
    pub span: SourceSpan,
}

impl Identifier {
    /// Create a new identifier
    pub fn new(name: WitBoundedString, span: SourceSpan) -> Self {
        Self { name, span }
    }
}

/// A complete WIT document AST (simplified version)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WitDocument {
    /// Optional package declaration
    pub package: Option<PackageDecl>,
    /// Use declarations at the top level
    #[cfg(feature = "std")]
    pub use_items: Vec<UseDecl>,
    /// Top-level items (interfaces, worlds, types)
    #[cfg(feature = "std")]
    pub items: Vec<TopLevelItem>,
    /// Source span of the entire document
    pub span: SourceSpan,
}

/// Package declaration
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PackageDecl {
    /// Package namespace (e.g., "wasi" in "wasi:cli")
    pub namespace: Identifier,
    /// Package name (e.g., "cli" in "wasi:cli")
    pub name: Identifier,
    /// Optional version
    pub version: Option<Version>,
    /// Source span
    pub span: SourceSpan,
}

/// Semantic version
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Version {
    /// Major version number
    pub major: u32,
    /// Minor version number
    pub minor: u32,
    /// Patch version number
    pub patch: u32,
    /// Optional pre-release identifier
    pub pre: Option<WitBoundedStringSmall>,
    /// Source span
    pub span: SourceSpan,
}

/// Use declaration for importing items
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UseDecl {
    /// The path being imported from
    pub path: UsePath,
    /// Optional renaming
    pub names: UseNames,
    /// Source span
    pub span: SourceSpan,
}

/// Path in a use declaration
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UsePath {
    /// Optional package prefix (e.g., "wasi:cli" in "use wasi:cli/types")
    pub package: Option<PackageRef>,
    /// Interface name
    pub interface: Identifier,
    /// Source span
    pub span: SourceSpan,
}

/// Package reference in a use path
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PackageRef {
    /// Namespace
    pub namespace: Identifier,
    /// Package name
    pub name: Identifier,
    /// Optional version
    pub version: Option<Version>,
    /// Source span
    pub span: SourceSpan,
}

/// Names being imported in a use declaration
#[derive(Debug, Clone, PartialEq)]
pub enum UseNames {
    /// Import all items (use foo/bar)
    All,
    /// Import specific items (use foo/bar.{a, b as c})
    #[cfg(feature = "std")]
    Items(Vec<UseItem>),
}

impl Default for UseNames {
    fn default() -> Self {
        Self::All
    }
}

/// A single item in a use declaration
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UseItem {
    /// Original name
    pub name: Identifier,
    /// Optional rename (for "as" syntax)
    pub as_name: Option<Identifier>,
    /// Source span
    pub span: SourceSpan,
}

/// Top-level items in a WIT document
#[derive(Debug, Clone, PartialEq)]
pub enum TopLevelItem {
    /// Type declaration
    Type(TypeDecl),
    /// Interface declaration
    Interface(InterfaceDecl),
    /// World declaration
    World(WorldDecl),
}

impl TopLevelItem {
    /// Get the source span of this item
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Type(t) => t.span,
            Self::Interface(i) => i.span,
            Self::World(w) => w.span,
        }
    }
}

/// Type declaration
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TypeDecl {
    /// Type name
    pub name: Identifier,
    /// Type definition
    pub def: TypeDef,
    /// Documentation comments
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// Type definition kinds
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef {
    /// Type alias (type foo = bar)
    Alias(TypeExpr),
    /// Record type
    Record(RecordType),
    /// Variant type
    Variant(VariantType),
    /// Enum type
    Enum(EnumType),
    /// Flags type
    Flags(FlagsType),
    /// Resource type
    Resource(ResourceType),
}

impl Default for TypeDef {
    fn default() -> Self {
        Self::Alias(TypeExpr::Primitive(PrimitiveType {
            kind: PrimitiveKind::String,
            span: SourceSpan::empty(),
        }))
    }
}

/// Type expression (references to types)
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// Primitive type
    Primitive(PrimitiveType),
    /// Named type reference
    Named(NamedType),
    /// List type
    #[cfg(feature = "std")]
    List(Box<TypeExpr>, SourceSpan),
    /// Option type
    #[cfg(feature = "std")]
    Option(Box<TypeExpr>, SourceSpan),
    /// Result type
    Result(ResultType),
    /// Tuple type
    Tuple(TupleType),
    /// Stream type (for async)
    #[cfg(feature = "std")]
    Stream(Box<TypeExpr>, SourceSpan),
    /// Future type (for async)
    #[cfg(feature = "std")]
    Future(Box<TypeExpr>, SourceSpan),
    /// Owned handle
    Own(Identifier, SourceSpan),
    /// Borrowed handle
    Borrow(Identifier, SourceSpan),
}

impl TypeExpr {
    /// Get the source span of this type expression
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Primitive(p) => p.span,
            Self::Named(n) => n.span,
            #[cfg(feature = "std")]
            Self::List(_, span)
            | Self::Option(_, span)
            | Self::Stream(_, span)
            | Self::Future(_, span) => *span,
            Self::Own(_, span) | Self::Borrow(_, span) => *span,
            Self::Result(r) => r.span,
            Self::Tuple(t) => t.span,
        }
    }
}

impl Default for TypeExpr {
    fn default() -> Self {
        Self::Primitive(PrimitiveType {
            kind: PrimitiveKind::String,
            span: SourceSpan::empty(),
        })
    }
}

/// Primitive types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrimitiveType {
    /// The primitive type kind
    pub kind: PrimitiveKind,
    /// Source span
    pub span: SourceSpan,
}

/// Primitive type kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveKind {
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
}

impl Default for PrimitiveKind {
    fn default() -> Self {
        Self::String
    }
}

/// Named type reference
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NamedType {
    /// Package reference (for qualified names)
    pub package: Option<PackageRef>,
    /// Type name
    pub name: Identifier,
    /// Source span
    pub span: SourceSpan,
}

/// Result type
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResultType {
    /// Success type
    #[cfg(feature = "std")]
    pub ok: Option<Box<TypeExpr>>,
    /// Error type
    #[cfg(feature = "std")]
    pub err: Option<Box<TypeExpr>>,
    /// Source span
    pub span: SourceSpan,
}

/// Tuple type
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TupleType {
    /// Tuple elements
    #[cfg(feature = "std")]
    pub types: Vec<TypeExpr>,
    /// Source span
    pub span: SourceSpan,
}

/// Record type definition
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecordType {
    /// Record fields
    #[cfg(feature = "std")]
    pub fields: Vec<RecordField>,
    /// Source span
    pub span: SourceSpan,
}

/// Record field
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecordField {
    /// Field name
    pub name: Identifier,
    /// Field type
    pub ty: TypeExpr,
    /// Documentation
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// Variant type definition
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VariantType {
    /// Variant cases
    #[cfg(feature = "std")]
    pub cases: Vec<VariantCase>,
    /// Source span
    pub span: SourceSpan,
}

/// Variant case
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VariantCase {
    /// Case name
    pub name: Identifier,
    /// Optional payload type
    pub ty: Option<TypeExpr>,
    /// Documentation
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// Enum type definition
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnumType {
    /// Enum cases
    #[cfg(feature = "std")]
    pub cases: Vec<EnumCase>,
    /// Source span
    pub span: SourceSpan,
}

/// Enum case
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnumCase {
    /// Case name
    pub name: Identifier,
    /// Documentation
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// Flags type definition
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlagsType {
    /// Flag values
    #[cfg(feature = "std")]
    pub flags: Vec<FlagValue>,
    /// Source span
    pub span: SourceSpan,
}

/// Flag value
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlagValue {
    /// Flag name
    pub name: Identifier,
    /// Documentation
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// Resource type definition
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResourceType {
    /// Resource methods
    #[cfg(feature = "std")]
    pub methods: Vec<ResourceMethod>,
    /// Source span
    pub span: SourceSpan,
}

/// Resource method
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResourceMethod {
    /// Method name
    pub name: Identifier,
    /// Method kind
    pub kind: ResourceMethodKind,
    /// Function signature
    pub func: Function,
    /// Documentation
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// Resource method kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceMethodKind {
    /// Constructor
    Constructor,
    /// Static method
    Static,
    /// Instance method
    Method,
}

impl Default for ResourceMethodKind {
    fn default() -> Self {
        Self::Method
    }
}

/// Interface declaration
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InterfaceDecl {
    /// Interface name
    pub name: Identifier,
    /// Interface items
    #[cfg(feature = "std")]
    pub items: Vec<InterfaceItem>,
    /// Documentation
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// Interface items
#[derive(Debug, Clone, PartialEq)]
pub enum InterfaceItem {
    /// Use declaration
    Use(UseDecl),
    /// Type declaration
    Type(TypeDecl),
    /// Function declaration
    Function(FunctionDecl),
}

impl InterfaceItem {
    /// Get the source span of this item
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Use(u) => u.span,
            Self::Type(t) => t.span,
            Self::Function(f) => f.span,
        }
    }
}

/// Function declaration
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FunctionDecl {
    /// Function name
    pub name: Identifier,
    /// Function signature
    pub func: Function,
    /// Documentation
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// Function signature
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Function {
    /// Parameters
    #[cfg(feature = "std")]
    pub params: Vec<Param>,
    /// Results
    pub results: FunctionResults,
    /// Whether this is async
    pub is_async: bool,
    /// Source span
    pub span: SourceSpan,
}

/// Function parameter
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Param {
    /// Parameter name
    pub name: Identifier,
    /// Parameter type
    pub ty: TypeExpr,
    /// Source span
    pub span: SourceSpan,
}

/// Function results
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionResults {
    /// No results
    None,
    /// Single unnamed result
    Single(TypeExpr),
    /// Named results
    #[cfg(feature = "std")]
    Named(Vec<NamedResult>),
}

impl Default for FunctionResults {
    fn default() -> Self {
        Self::None
    }
}

/// Named function result
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NamedResult {
    /// Result name
    pub name: Identifier,
    /// Result type
    pub ty: TypeExpr,
    /// Source span
    pub span: SourceSpan,
}

/// World declaration
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorldDecl {
    /// World name
    pub name: Identifier,
    /// World items
    #[cfg(feature = "std")]
    pub items: Vec<WorldItem>,
    /// Documentation
    pub docs: Option<Documentation>,
    /// Source span
    pub span: SourceSpan,
}

/// World items
#[derive(Debug, Clone, PartialEq)]
pub enum WorldItem {
    /// Use declaration
    Use(UseDecl),
    /// Type declaration
    Type(TypeDecl),
    /// Import
    Import(ImportItem),
    /// Export
    Export(ExportItem),
    /// Include another world
    Include(IncludeItem),
}

impl WorldItem {
    /// Get the source span of this item
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Use(u) => u.span,
            Self::Type(t) => t.span,
            Self::Import(i) => i.span,
            Self::Export(e) => e.span,
            Self::Include(i) => i.span,
        }
    }
}

/// Import item in a world
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportItem {
    /// Import name
    pub name: Identifier,
    /// Import kind
    pub kind: ImportExportKind,
    /// Source span
    pub span: SourceSpan,
}

/// Export item in a world
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExportItem {
    /// Export name
    pub name: Identifier,
    /// Export kind
    pub kind: ImportExportKind,
    /// Source span
    pub span: SourceSpan,
}

/// Include item in a world
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IncludeItem {
    /// World being included
    pub world: NamedType,
    /// Optional include specifier
    pub with: Option<IncludeWith>,
    /// Source span
    pub span: SourceSpan,
}

/// Include with specifier
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IncludeWith {
    /// Renamings
    #[cfg(feature = "std")]
    pub items: Vec<IncludeRename>,
    /// Source span
    pub span: SourceSpan,
}

/// Include rename item
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IncludeRename {
    /// Original name
    pub from: Identifier,
    /// New name
    pub to: Identifier,
    /// Source span
    pub span: SourceSpan,
}

/// Import/export kinds
#[derive(Debug, Clone, PartialEq)]
pub enum ImportExportKind {
    /// Function
    Function(Function),
    /// Interface reference
    Interface(NamedType),
    /// Type reference
    Type(TypeExpr),
}

impl Default for ImportExportKind {
    fn default() -> Self {
        Self::Type(TypeExpr::default())
    }
}

/// Documentation comments
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Documentation {
    /// Documentation lines
    #[cfg(feature = "std")]
    pub lines: Vec<WitBoundedString>,
    /// Source span
    pub span: SourceSpan,
}

// Display implementations for better debugging
impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name.as_str().unwrap_or("<invalid>"))
    }
}

impl fmt::Display for PrimitiveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => write!(f, "bool"),
            Self::U8 => write!(f, "u8"),
            Self::U16 => write!(f, "u16"),
            Self::U32 => write!(f, "u32"),
            Self::U64 => write!(f, "u64"),
            Self::S8 => write!(f, "s8"),
            Self::S16 => write!(f, "s16"),
            Self::S32 => write!(f, "s32"),
            Self::S64 => write!(f, "s64"),
            Self::F32 => write!(f, "f32"),
            Self::F64 => write!(f, "f64"),
            Self::Char => write!(f, "char"),
            Self::String => write!(f, "string"),
        }
    }
}
