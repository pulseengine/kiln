//! WebAssembly Component Model format.
//!
//! This module provides types and utilities for working with the WebAssembly
//! Component Model binary format.

// Use crate-level type aliases for collection types
#[cfg(feature = "std")]
use std::{boxed::Box, format};

// Binary std/no_std choice
#[cfg(feature = "std")]
macro_rules! validation_error {
    ($($arg:tt)*) => {
        crate::error::validation_error_dynamic(format!($($arg)*))
    };
}

#[cfg(not(any(feature = "std")))]
macro_rules! validation_error {
    ($($arg:tt)*) => {
        crate::error::validation_error("validation error (details unavailable in no_std)")
    };
}

use wrt_error::{Error, Result};
// Binary std/no_std choice
#[cfg(feature = "std")]
pub use wrt_foundation::component_value::ValType;

// Provide a simple stub for ValType in no_std mode
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValType {
    Bool,
    S8,
    U8,
    S16,
    U16,
    S32,
    U32,
    S64,
    U64,
    F32,
    F64,
    Char,
    String,
}
#[cfg(not(any(feature = "std")))]
use wrt_foundation::NoStdProvider;
pub use wrt_foundation::resource::{
    ResourceDrop, ResourceNew, ResourceRep, ResourceRepresentation,
};

#[cfg(not(any(feature = "std")))]
use crate::{MAX_TYPE_RECURSION_DEPTH, WasmString, WasmVec};
#[cfg(feature = "std")]
use crate::{String, Vec};
use crate::{module::Module, types::ValueType, validation::Validatable};

// Conditional type aliases for collection types
#[cfg(feature = "std")]
type ComponentString = String;
#[cfg(not(any(feature = "std")))]
type ComponentString = WasmString<NoStdProvider<512>>;

#[cfg(feature = "std")]
type ComponentVec<T> = Vec<T>;
#[cfg(not(any(feature = "std")))]
type ComponentVec<T> = WasmVec<T, NoStdProvider<1024>>;

/// WebAssembly Component Model component definition
#[derive(Debug, Clone)]
pub struct Component {
    /// Component name (if available from name section)
    pub name: Option<ComponentString>,
    /// Core modules included in this component
    pub modules: ComponentVec<Module>,
    /// Core instances defined in this component
    pub core_instances: ComponentVec<CoreInstance>,
    /// Core types defined in this component
    pub core_types: ComponentVec<CoreType>,
    /// Nested components
    pub components: ComponentVec<Component>,
    /// Component instances
    pub instances: ComponentVec<Instance>,
    /// Component aliases
    pub aliases: ComponentVec<Alias>,
    /// Component types
    pub types: ComponentVec<ComponentType>,
    /// Canonical function conversions
    pub canonicals: ComponentVec<Canon>,
    /// Component start function
    pub start: Option<Start>,
    /// Component imports
    pub imports: ComponentVec<Import>,
    /// Component exports
    pub exports: ComponentVec<Export>,
    /// Component values
    pub values: ComponentVec<Value>,
    /// Original binary (if available)
    pub binary: Option<ComponentVec<u8>>,
}

impl Default for Component {
    fn default() -> Self {
        Self::new()
    }
}

impl Component {
    /// Create a new empty component
    pub fn new() -> Self {
        Self {
            name: None,
            modules: Self::new_vec(),
            core_instances: Self::new_vec(),
            core_types: Self::new_vec(),
            components: Self::new_vec(),
            instances: Self::new_vec(),
            aliases: Self::new_vec(),
            types: Self::new_vec(),
            canonicals: Self::new_vec(),
            start: None,
            imports: Self::new_vec(),
            exports: Self::new_vec(),
            values: Self::new_vec(),
            binary: None,
        }
    }

    /// Helper to create a new ComponentVec
    #[cfg(feature = "std")]
    fn new_vec<T>() -> ComponentVec<T> {
        Vec::new()
    }

    /// Helper to create a new ComponentVec for no_std
    #[cfg(not(any(feature = "std")))]
    fn new_vec<T>() -> ComponentVec<T> {
        let provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Format
        )
        .unwrap_or_else(|_| panic!("Failed to allocate memory provider"));
        WasmVec::new(provider).unwrap_or_else(|_| panic!("Failed to create WasmVec"))
    }
}

impl Validatable for Component {
    fn validate(&self) -> Result<()> {
        // Validate component name if present
        if let Some(name) = &self.name {
            if name.is_empty() {
                return Err(Error::validation_error("Component name cannot be empty"));
            }
        }

        // Validate all child elements
        self.modules.validate()?;
        self.core_instances.validate()?;
        self.core_types.validate()?;
        self.components.validate()?;
        self.instances.validate()?;
        self.aliases.validate()?;
        self.types.validate()?;
        self.canonicals.validate()?;
        self.start.validate()?;
        self.imports.validate()?;
        self.exports.validate()?;
        self.values.validate()?;

        Ok(())
    }
}

/// Core WebAssembly instance definition in a component
#[derive(Debug, Clone)]
pub struct CoreInstance {
    /// Instance expression
    pub instance_expr: CoreInstanceExpr,
}

impl Validatable for CoreInstance {
    fn validate(&self) -> Result<()> {
        match &self.instance_expr {
            CoreInstanceExpr::ModuleReference {
                module_idx,
                arg_refs,
            } => {
                // Basic validation: module_idx should be reasonable
                if *module_idx > 10000 {
                    // Arbitrary reasonable limit
                    return Err(validation_error!(
                        "Module index {} seems unreasonably large",
                        module_idx
                    ));
                }

                // Validate arg references
                for arg_ref in arg_refs {
                    if arg_ref.name.is_empty() {
                        return Err(Error::validation_error(
                            "Arg reference name cannot be empty",
                        ));
                    }
                }

                Ok(())
            },
            CoreInstanceExpr::InlineExports(exports) => {
                // Validate exports
                for export in exports {
                    if export.name.is_empty() {
                        return Err(Error::validation_error(
                            "Inline export name cannot be empty",
                        ));
                    }
                    // Reasonable index limit
                    if export.idx > 100_000 {
                        return Err(validation_error!(
                            "Export index {} seems unreasonably large",
                            export.idx
                        ));
                    }
                }

                Ok(())
            },
        }
    }
}

/// Core WebAssembly instance expression (format representation only)
#[derive(Debug, Clone)]
pub enum CoreInstanceExpr {
    /// Reference to a module for instantiation (format-only, runtime handles
    /// actual instantiation)
    ModuleReference {
        /// Module index
        module_idx: u32,
        /// Format-only argument references
        arg_refs: Vec<CoreArgReference>,
    },
    /// Collection of inlined exports
    InlineExports(Vec<CoreInlineExport>),
}

/// Core WebAssembly argument reference (format representation only)
#[derive(Debug, Clone)]
pub struct CoreArgReference {
    /// Name of the argument (import module/name as specified in the wasm module)
    pub name: String,
    /// Sort/kind of the argument (0x00=Func, 0x01=Table, 0x02=Mem, 0x03=Global, 0x12=Instance, ...)
    /// This determines which index space `idx` refers to in the component.
    pub kind: u8,
    /// Index in the corresponding component index space (not the instance index!)
    /// For kind=0x12 (Instance), this IS the instance index.
    /// For kind=0x00 (Func), this is a function index in the component's core func space.
    /// For kind=0x01 (Table), this is a table index in the component's core table space.
    pub idx: u32,
}

/// Core WebAssembly inlined export
#[derive(Debug, Clone)]
pub struct CoreInlineExport {
    /// Export name
    pub name: String,
    /// Export reference
    pub sort: CoreSort,
    /// Index within the sort
    pub idx: u32,
}

/// Core WebAssembly sort kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreSort {
    /// Function reference
    Function,
    /// Table reference
    Table,
    /// Memory reference
    Memory,
    /// Global reference
    Global,
    /// Type reference
    Type,
    /// Module reference
    Module,
    /// Instance reference
    Instance,
}

/// Core WebAssembly type definition in a component
#[derive(Debug, Clone)]
pub struct CoreType {
    /// Type definition
    pub definition: CoreTypeDefinition,
}

impl Validatable for CoreType {
    fn validate(&self) -> Result<()> {
        match &self.definition {
            CoreTypeDefinition::Function { params, results } => {
                // Basic validation: reasonable limits on params and results
                if params.len() > 1000 {
                    return Err(validation_error!(
                        "Function has too many parameters ({})",
                        params.len()
                    ));
                }

                if results.len() > 1000 {
                    return Err(validation_error!(
                        "Function has too many results ({})",
                        results.len()
                    ));
                }

                Ok(())
            },
            CoreTypeDefinition::Module { imports, exports } => {
                // Validate imports
                for (namespace, name, _) in imports {
                    if namespace.is_empty() {
                        return Err(Error::validation_error("Import namespace cannot be empty"));
                    }
                    if name.is_empty() {
                        return Err(Error::validation_error("Import name cannot be empty"));
                    }
                }

                // Validate exports
                for (name, _) in exports {
                    if name.is_empty() {
                        return Err(Error::validation_error("Export name cannot be empty"));
                    }
                }

                Ok(())
            },
        }
    }
}

/// Core WebAssembly type definition
#[derive(Debug, Clone)]
pub enum CoreTypeDefinition {
    /// Function type
    Function {
        /// Parameter types
        params: Vec<ValueType>,
        /// Result types
        results: Vec<ValueType>,
    },
    /// Module type
    Module {
        /// Module imports
        imports: Vec<(String, String, CoreExternType)>,
        /// Module exports
        exports: Vec<(String, CoreExternType)>,
    },
}

/// Core WebAssembly external type
#[derive(Debug, Clone)]
pub enum CoreExternType {
    /// Function type
    Function {
        /// Parameter types
        params: Vec<ValueType>,
        /// Result types
        results: Vec<ValueType>,
    },
    /// Table type
    Table {
        /// Element type
        element_type: ValueType,
        /// Minimum size
        min: u32,
        /// Maximum size (optional)
        max: Option<u32>,
    },
    /// Memory type
    Memory {
        /// Minimum size in pages
        min: u32,
        /// Maximum size in pages (optional)
        max: Option<u32>,
        /// Whether the memory is shared
        shared: bool,
    },
    /// Global type
    Global {
        /// Value type
        value_type: ValueType,
        /// Whether the global is mutable
        mutable: bool,
    },
}

/// Component instance definition
#[derive(Debug, Clone)]
pub struct Instance {
    /// Instance expression
    pub instance_expr: InstanceExpr,
}

/// Component instance expression (format representation only)
#[derive(Debug, Clone)]
pub enum InstanceExpr {
    /// Reference to a component for instantiation (format-only, runtime handles
    /// actual instantiation)
    ComponentReference {
        /// Component index
        component_idx: u32,
        /// Format-only argument references
        arg_refs: Vec<InstantiateArgReference>,
    },
    /// Collection of inlined exports
    InlineExports(Vec<InlineExport>),
}

/// Component instantiation argument reference (format representation only)
#[derive(Debug, Clone)]
pub struct InstantiateArgReference {
    /// Name of the argument
    pub name: String,
    /// Sort of the referenced item (format-only)
    pub sort: Sort,
    /// Index within the sort (format-only)
    pub idx: u32,
}

/// Component inlined export
#[derive(Debug, Clone)]
pub struct InlineExport {
    /// Export name
    pub name: String,
    /// Export reference
    pub sort: Sort,
    /// Index within the sort
    pub idx: u32,
}

/// Component sort kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sort {
    /// Core reference
    Core(CoreSort),
    /// Function reference
    Function,
    /// Value reference
    Value,
    /// Type reference
    Type,
    /// Component reference
    Component,
    /// Instance reference
    Instance,
}

/// Component alias definition
#[derive(Debug, Clone)]
pub struct Alias {
    /// Alias target
    pub target: AliasTarget,
    /// Destination index in the appropriate index space (computed during parsing)
    /// This is the index this alias occupies, accounting for intermixed canon definitions
    pub dest_idx: Option<u32>,
}

/// Component alias target
#[derive(Debug, Clone)]
pub enum AliasTarget {
    /// Core WebAssembly export from an instance
    CoreInstanceExport {
        /// Instance index
        instance_idx: u32,
        /// Export name
        name: String,
        /// Kind of the target
        kind: CoreSort,
    },
    /// Export from a component instance
    InstanceExport {
        /// Instance index
        instance_idx: u32,
        /// Export name
        name: String,
        /// Kind of the target
        kind: Sort,
    },
    /// Outer definition from an enclosing component (forwarding from parent)
    Outer {
        /// Count of components to traverse outward
        count: u32,
        /// Kind of the target
        kind: Sort,
        /// Index within the kind
        idx: u32,
    },
}

/// Component type definition
#[derive(Debug, Clone)]
pub struct ComponentType {
    /// Type definition
    pub definition: ComponentTypeDefinition,
}

/// Component type definition
#[derive(Debug, Clone)]
pub enum ComponentTypeDefinition {
    /// Component type
    Component {
        /// Component imports
        imports: Vec<(String, String, ExternType)>,
        /// Component exports
        exports: Vec<(String, ExternType)>,
    },
    /// Instance type
    Instance {
        /// Instance exports
        exports: Vec<(String, ExternType)>,
    },
    /// Function type
    Function {
        /// Parameter types
        params: Vec<(String, FormatValType)>,
        /// Result types
        results: Vec<FormatValType>,
    },
    /// Value type
    Value(FormatValType),
    /// Resource type
    Resource {
        /// Resource representation type
        representation: ResourceRepresentation,
        /// Whether the resource is nullable
        nullable: bool,
    },
}

/// Component external type
#[derive(Debug, Clone, PartialEq)]
pub enum ExternType {
    /// Core module type
    Module {
        /// Type index
        type_idx: u32,
    },
    /// Function type
    Function {
        /// Parameter types
        params: Vec<(String, FormatValType)>,
        /// Result types
        results: Vec<FormatValType>,
    },
    /// Value type
    Value(FormatValType),
    /// Type reference
    Type(u32),
    /// Instance type
    Instance {
        /// Instance exports
        exports: Vec<(String, ExternType)>,
    },
    /// Component type
    Component {
        /// Type index
        type_idx: u32,
    },
}

/// Type reference index for recursive types (replaces Box<T>)
pub type TypeRef = u32;

/// Binary std/no_std choice
#[cfg(not(any(feature = "std")))]
#[derive(Debug, Clone)]
pub struct TypeRegistry<P: wrt_foundation::MemoryProvider = NoStdProvider<1024>> {
    /// Type definitions stored in a bounded vector
    types: WasmVec<FormatValType<P>, P>,
    /// Next available type reference
    next_ref: TypeRef,
}

#[cfg(not(any(feature = "std")))]
impl<P: wrt_foundation::MemoryProvider + Clone + Default> TypeRegistry<P> {
    /// Create a new type registry
    pub fn new() -> Result<Self, wrt_foundation::bounded::CapacityError> {
        Ok(Self {
            types: WasmVec::new(P::default())?,
            next_ref: 0,
        })
    }

    /// Add a type to the registry and return its reference
    pub fn add_type(
        &mut self,
        val_type: FormatValType<P>,
    ) -> Result<TypeRef, wrt_foundation::bounded::CapacityError> {
        let type_ref = self.next_ref;
        self.types.push(val_type)?;
        self.next_ref += 1;
        Ok(type_ref)
    }

    /// Get a type by reference
    pub fn get_type(&self, type_ref: TypeRef) -> Option<&FormatValType<P>> {
        self.types.get(type_ref as usize)
    }

    /// Get mutable reference to a type
    pub fn get_type_mut(&mut self, type_ref: TypeRef) -> Option<&mut FormatValType<P>> {
        self.types.get_mut(type_ref as usize)
    }
}

/// Component Model value types - Pure No_std Version
#[cfg(not(any(feature = "std")))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatValType<P: wrt_foundation::MemoryProvider = NoStdProvider<1024>> {
    /// Boolean type
    Bool,
    /// 8-bit signed integer
    S8,
    /// 8-bit unsigned integer
    U8,
    /// 16-bit signed integer
    S16,
    /// 16-bit unsigned integer
    U16,
    /// 32-bit signed integer
    S32,
    /// 32-bit unsigned integer
    U32,
    /// 64-bit signed integer
    S64,
    /// 64-bit unsigned integer
    U64,
    /// 32-bit float
    F32,
    /// 64-bit float
    F64,
    /// Unicode character
    Char,
    /// String type
    String,
    /// Reference type
    Ref(u32),
    /// Record type with named fields (using type references)
    Record(WasmVec<(WasmString<P>, TypeRef), P>),
    /// Variant type (using type references)  
    Variant(WasmVec<(WasmString<P>, Option<TypeRef>), P>),
    /// List type (reference to element type)
    List(TypeRef),
    /// Fixed-length list type with element type and length
    FixedList(TypeRef, u32),
    /// Tuple type (references to element types)
    Tuple(WasmVec<TypeRef, P>),
    /// Flags type
    Flags(WasmVec<WasmString<P>, P>),
    /// Enum type
    Enum(WasmVec<WasmString<P>, P>),
    /// Option type (reference to inner type)
    Option(TypeRef),
    /// Result type (reference to ok/error types)
    Result(TypeRef),
    /// Own a resource
    Own(u32),
    /// Borrow a resource
    Borrow(u32),
    /// Void/empty type
    Void,
    /// `Error` context type
    ErrorContext,
}

/// Component Model value types - With Allocation
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatValType {
    /// Boolean type
    Bool,
    /// 8-bit signed integer
    S8,
    /// 8-bit unsigned integer
    U8,
    /// 16-bit signed integer
    S16,
    /// 16-bit unsigned integer
    U16,
    /// 32-bit signed integer
    S32,
    /// 32-bit unsigned integer
    U32,
    /// 64-bit signed integer
    S64,
    /// 64-bit unsigned integer
    U64,
    /// 32-bit float
    F32,
    /// 64-bit float
    F64,
    /// Unicode character
    Char,
    /// String type
    String,
    /// Reference type
    Ref(u32),
    /// Record type with named fields
    Record(Vec<(String, FormatValType)>),
    /// Variant type
    Variant(Vec<(String, Option<FormatValType>)>),
    /// List type
    List(Box<FormatValType>),
    /// Fixed-length list type with element type and length
    FixedList(Box<FormatValType>, u32),
    /// Tuple type
    Tuple(Vec<FormatValType>),
    /// Flags type
    Flags(Vec<String>),
    /// Enum type
    Enum(Vec<String>),
    /// Option type
    Option(Box<FormatValType>),
    /// Result type (can contain ok or error)
    Result(Box<FormatValType>),
    /// Own a resource
    Own(u32),
    /// Borrow a resource
    Borrow(u32),
    /// Void/empty type
    Void,
    /// `Error` context type
    ErrorContext,
}

/// Canonical function conversion
#[derive(Debug, Clone)]
pub struct Canon {
    /// Canonical operation
    pub operation: CanonOperation,
}

/// Canonical operation
#[derive(Debug, Clone)]
pub enum CanonOperation {
    /// Lift a core function to the component ABI
    Lift {
        /// Core function index
        func_idx: u32,
        /// Type index for the lifted function
        type_idx: u32,
        /// Options for lifting
        options: LiftOptions,
    },
    /// Lower a component function to the core ABI
    Lower {
        /// Component function index
        func_idx: u32,
        /// Options for lowering
        options: LowerOptions,
    },
    /// Resource operations
    Resource(FormatResourceOperation),
    /// Binary std/no_std choice
    Realloc {
        /// Binary std/no_std choice
        alloc_func_idx: u32,
        /// Memory index to use
        memory_idx: u32,
    },
    /// Post-return operation (cleanup)
    PostReturn {
        /// Function index for post-return cleanup
        func_idx: u32,
    },
    /// Memory copy operation (optional optimization)
    MemoryCopy {
        /// Source memory index
        src_memory_idx: u32,
        /// Destination memory index
        dst_memory_idx: u32,
        /// Function index for the copy operation
        func_idx: u32,
    },
    /// Async operation (stackful lift)
    Async {
        /// Function index for the async operation
        func_idx: u32,
        /// Type index for the async function
        type_idx: u32,
        /// Options for async operations
        options: AsyncOptions,
    },
}

/// Options for lifting operations
#[derive(Debug, Clone)]
pub struct LiftOptions {
    /// Memory index to use for string/list conversions
    pub memory_idx: Option<u32>,
    /// String encoding to use
    pub string_encoding: Option<StringEncoding>,
    /// Binary std/no_std choice
    pub realloc_func_idx: Option<u32>,
    /// Post-return function index (optional)
    pub post_return_func_idx: Option<u32>,
    /// Whether this is an async lift
    pub is_async: bool,
}

/// Options for lowering operations
#[derive(Debug, Clone)]
pub struct LowerOptions {
    /// Memory index to use for string/list conversions
    pub memory_idx: Option<u32>,
    /// String encoding to use
    pub string_encoding: Option<StringEncoding>,
    /// Binary std/no_std choice
    pub realloc_func_idx: Option<u32>,
    /// Whether this is an async lower
    pub is_async: bool,
    /// `Error` handling mode
    pub error_mode: Option<ErrorMode>,
}

/// Options for async operations
#[derive(Debug, Clone)]
pub struct AsyncOptions {
    /// Memory index to use
    pub memory_idx: u32,
    /// Binary std/no_std choice
    pub realloc_func_idx: Option<u32>,
    /// String encoding to use
    pub string_encoding: Option<StringEncoding>,
}

/// String encoding options
#[derive(Debug, Clone)]
pub enum StringEncoding {
    /// UTF-8 encoding
    UTF8,
    /// UTF-16 encoding
    UTF16,
    /// Latin-1 encoding
    Latin1,
    /// ASCII encoding
    ASCII,
}

/// `Error` handling modes
#[derive(Debug, Clone)]
pub enum ErrorMode {
    /// Convert errors to exceptions
    ThrowOnError,
    /// Convert exceptions to errors
    CatchExceptions,
    /// Pass through errors/exceptions
    Passthrough,
}

/// Component start function
#[derive(Debug, Clone)]
pub struct Start {
    /// Function index
    pub func_idx: u32,
    /// Value arguments
    pub args: Vec<u32>,
    /// Number of results
    pub results: u32,
}

/// Import definition in a component
#[derive(Debug, Clone)]
pub struct Import {
    /// Import name in namespace.name format
    pub name: ImportName,
    /// Type of the import
    pub ty: ExternType,
}

/// Import name with support for nested namespaces
#[derive(Debug, Clone)]
pub struct ImportName {
    /// Namespace of the import
    pub namespace: String,
    /// Name of the import
    pub name: String,
    /// Nested namespaces (if any)
    pub nested: Vec<String>,
    /// Package reference (if any)
    pub package: Option<PackageReference>,
}

/// Package reference for imports
#[derive(Debug, Clone)]
pub struct PackageReference {
    /// Package name
    pub name: String,
    /// Package version
    pub version: Option<String>,
    /// Package hash (for content verification)
    pub hash: Option<String>,
}

/// Export definition in a component
#[derive(Debug, Clone)]
pub struct Export {
    /// Export name in "name" format
    pub name: ExportName,
    /// Sort of the exported item
    pub sort: Sort,
    /// Index within the sort
    pub idx: u32,
    /// Declared type (optional)
    pub ty: Option<ExternType>,
}

/// Export name with support for nested namespaces
#[derive(Debug, Clone)]
pub struct ExportName {
    /// Basic name
    pub name: String,
    /// Whether this export is a resource
    pub is_resource: bool,
    /// Semver compatibility string
    pub semver: Option<String>,
    /// Integrity hash for content verification
    pub integrity: Option<String>,
    /// Nested namespaces (if any)
    pub nested: Vec<String>,
}

impl ImportName {
    /// Create a new import name with just namespace and name
    #[cfg(feature = "std")]
    pub fn new(namespace: String, name: String) -> Self {
        Self {
            namespace,
            name,
            nested: Vec::new(),
            package: None,
        }
    }

    /// Create a new import name with nested namespaces
    #[cfg(feature = "std")]
    pub fn with_nested(namespace: String, name: String, nested: Vec<String>) -> Self {
        Self {
            namespace,
            name,
            nested,
            package: None,
        }
    }

    /// Add package reference to an import name
    #[cfg(feature = "std")]
    pub fn with_package(mut self, package: PackageReference) -> Self {
        self.package = Some(package);
        self
    }

    /// Get the full import path as a string
    #[cfg(feature = "std")]
    pub fn full_path(&self) -> String {
        let mut path = format!("{}.{}", self.namespace, self.name);
        for nested in &self.nested {
            path.push_str(&format!(".{}", nested));
        }
        path
    }
}

impl ExportName {
    /// Create a new export name
    pub fn new(name: String) -> Self {
        Self {
            name,
            is_resource: false,
            semver: None,
            integrity: None,
            nested: Vec::new(),
        }
    }

    /// Create a new export name with nested namespaces
    pub fn with_nested(name: String, nested: Vec<String>) -> Self {
        Self {
            name,
            is_resource: false,
            semver: None,
            integrity: None,
            nested,
        }
    }

    /// Mark as a resource export
    pub fn as_resource(mut self) -> Self {
        self.is_resource = true;
        self
    }

    /// Add semver compatibility information
    pub fn with_semver(mut self, semver: String) -> Self {
        self.semver = Some(semver);
        self
    }

    /// Add integrity hash for content verification
    pub fn with_integrity(mut self, integrity: String) -> Self {
        self.integrity = Some(integrity);
        self
    }

    /// Get the full export path as a string
    pub fn full_path(&self) -> String {
        let mut path = self.name.clone();
        for nested in &self.nested {
            path.push_str(&format!(".{}", nested));
        }
        path
    }
}

/// Component value definition
#[derive(Debug, Clone)]
pub struct Value {
    /// Type of the value
    pub ty: FormatValType,
    /// Encoded value data
    pub data: Vec<u8>,
    /// Value expression (if available)
    pub expression: Option<ValueExpression>,
    /// Value name (if available from custom sections)
    pub name: Option<String>,
}

/// Value expression types
#[derive(Debug, Clone)]
pub enum ValueExpression {
    /// Reference to an item in component
    ItemRef {
        /// Sort of the referenced item
        sort: Sort,
        /// Index within the sort
        idx: u32,
    },
    /// Global initialization expression
    GlobalInit {
        /// Global index
        global_idx: u32,
    },
    /// Function call expression
    FunctionCall {
        /// Function index
        func_idx: u32,
        /// Arguments (indices to other values)
        args: Vec<u32>,
    },
    /// Direct constant value
    Const(ConstValue),
}

/// Constant value types
#[derive(Debug, Clone)]
pub enum ConstValue {
    /// Boolean constant
    Bool(bool),
    /// 8-bit signed integer
    S8(i8),
    /// 8-bit unsigned integer
    U8(u8),
    /// 16-bit signed integer
    S16(i16),
    /// 16-bit unsigned integer
    U16(u16),
    /// 32-bit signed integer
    S32(i32),
    /// 32-bit unsigned integer
    U32(u32),
    /// 64-bit signed integer
    S64(i64),
    /// 64-bit unsigned integer
    U64(u64),
    /// 32-bit float
    F32(f32),
    /// 64-bit float
    F64(f64),
    /// Character constant
    Char(char),
    /// String constant
    String(String),
    /// Empty null constant
    Null,
}

impl Validatable for Instance {
    fn validate(&self) -> Result<()> {
        match &self.instance_expr {
            InstanceExpr::ComponentReference {
                component_idx,
                arg_refs,
            } => {
                // Basic validation: component_idx should be reasonable
                if *component_idx > 10000 {
                    // Arbitrary reasonable limit
                    return Err(validation_error!(
                        "Component index {} seems unreasonably large",
                        component_idx
                    ));
                }

                // Validate arg references
                for arg_ref in arg_refs {
                    if arg_ref.name.is_empty() {
                        return Err(Error::validation_error(
                            "Arg reference name cannot be empty",
                        ));
                    }
                }

                Ok(())
            },
            InstanceExpr::InlineExports(exports) => {
                // Validate exports
                for export in exports {
                    if export.name.is_empty() {
                        return Err(Error::validation_error(
                            "Inline export name cannot be empty",
                        ));
                    }
                }

                Ok(())
            },
        }
    }
}

impl Validatable for Alias {
    fn validate(&self) -> Result<()> {
        match &self.target {
            AliasTarget::CoreInstanceExport {
                instance_idx, name, ..
            } => {
                if *instance_idx > 10000 {
                    return Err(validation_error!(
                        "Instance index {} seems unreasonably large",
                        instance_idx
                    ));
                }

                if name.is_empty() {
                    return Err(Error::validation_error("Export name cannot be empty"));
                }

                Ok(())
            },
            AliasTarget::InstanceExport {
                instance_idx, name, ..
            } => {
                if *instance_idx > 10000 {
                    return Err(validation_error!(
                        "Instance index {} seems unreasonably large",
                        instance_idx
                    ));
                }

                if name.is_empty() {
                    return Err(Error::validation_error("Export name cannot be empty"));
                }

                Ok(())
            },
            AliasTarget::Outer { count, idx, .. } => {
                if *count > 10 {
                    return Err(validation_error!(
                        "Outer count {} seems unreasonably large",
                        count
                    ));
                }

                if *idx > 10000 {
                    return Err(validation_error!("Index {} seems unreasonably large", idx));
                }

                Ok(())
            },
        }
    }
}

impl Validatable for ComponentType {
    fn validate(&self) -> Result<()> {
        match &self.definition {
            ComponentTypeDefinition::Component { imports, exports } => {
                // Validate imports
                for (namespace, name, _) in imports {
                    if namespace.is_empty() {
                        return Err(Error::validation_error("Import namespace cannot be empty"));
                    }
                    if name.is_empty() {
                        return Err(Error::validation_error("Import name cannot be empty"));
                    }
                }

                // Validate exports
                for (name, _) in exports {
                    if name.is_empty() {
                        return Err(Error::validation_error("Export name cannot be empty"));
                    }
                }

                Ok(())
            },
            ComponentTypeDefinition::Instance { exports } => {
                // Validate exports
                for (name, _) in exports {
                    if name.is_empty() {
                        return Err(Error::validation_error("Export name cannot be empty"));
                    }
                }

                Ok(())
            },
            ComponentTypeDefinition::Function { params, results } => {
                // Basic validation: reasonable limits on params and results
                if params.len() > 1000 {
                    return Err(validation_error!(
                        "Function has too many parameters ({})",
                        params.len()
                    ));
                }

                // Check param names
                for (name, _) in params {
                    if name.is_empty() {
                        return Err(Error::validation_error("Parameter name cannot be empty"));
                    }
                }

                if results.len() > 1000 {
                    return Err(validation_error!(
                        "Function has too many results ({})",
                        results.len()
                    ));
                }

                Ok(())
            },
            ComponentTypeDefinition::Value(_) => {
                // Simple value types don't need further validation
                Ok(())
            },
            ComponentTypeDefinition::Resource { .. } => {
                // Resource types are validated elsewhere
                Ok(())
            },
        }
    }
}

impl Validatable for Canon {
    fn validate(&self) -> Result<()> {
        match &self.operation {
            CanonOperation::Lift {
                func_idx, type_idx, ..
            } => {
                if *func_idx > 10000 {
                    return Err(validation_error!(
                        "Function index {} seems unreasonably large",
                        func_idx
                    ));
                }

                if *type_idx > 10000 {
                    return Err(validation_error!(
                        "Type index {} seems unreasonably large",
                        type_idx
                    ));
                }

                Ok(())
            },
            CanonOperation::Lower { func_idx, .. } => {
                if *func_idx > 10000 {
                    return Err(validation_error!(
                        "Function index {} seems unreasonably large",
                        func_idx
                    ));
                }

                Ok(())
            },
            // Other operations have simpler validation requirements
            _ => Ok(()),
        }
    }
}

impl Validatable for Start {
    fn validate(&self) -> Result<()> {
        if self.func_idx > 10000 {
            return Err(validation_error!(
                "Function index {} seems unreasonably large",
                self.func_idx
            ));
        }

        if self.args.len() > 1000 {
            return Err(validation_error!(
                "Start function has too many arguments ({})",
                self.args.len()
            ));
        }

        if self.results > 1000 {
            return Err(validation_error!(
                "Start function has too many results ({})",
                self.results
            ));
        }

        Ok(())
    }
}

impl Validatable for Import {
    fn validate(&self) -> Result<()> {
        // Validate import name
        if self.name.namespace.is_empty() {
            return Err(Error::validation_error("Import namespace cannot be empty"));
        }

        if self.name.name.is_empty() {
            return Err(Error::validation_error("Import name cannot be empty"));
        }

        // Nested namespaces should not be empty strings
        for nested in &self.name.nested {
            if nested.is_empty() {
                return Err(Error::validation_error("Nested namespace cannot be empty"));
            }
        }

        // Validate package reference if present
        if let Some(pkg) = &self.name.package {
            if pkg.name.is_empty() {
                return Err(Error::validation_error("Package name cannot be empty"));
            }
        }

        Ok(())
    }
}

impl Validatable for Export {
    fn validate(&self) -> Result<()> {
        // Validate export name
        if self.name.name.is_empty() {
            return Err(Error::validation_error("Export name cannot be empty"));
        }

        // Nested namespaces should not be empty strings
        for nested in &self.name.nested {
            if nested.is_empty() {
                return Err(Error::validation_error("Nested namespace cannot be empty"));
            }
        }

        // Index should be reasonable
        if self.idx > 10000 {
            return Err(validation_error!(
                "Export index {} seems unreasonably large",
                self.idx
            ));
        }

        Ok(())
    }
}

impl Validatable for Value {
    fn validate(&self) -> Result<()> {
        // Validate data size (should be reasonable)
        if self.data.len() > 1_000_000 {
            return Err(validation_error!(
                "Value data size {} seems unreasonably large",
                self.data.len()
            ));
        }

        // Check value expression if present
        if let Some(expr) = &self.expression {
            match expr {
                ValueExpression::ItemRef { idx, .. } => {
                    if *idx > 10000 {
                        return Err(validation_error!(
                            "Item reference index {} seems unreasonably large",
                            idx
                        ));
                    }
                },
                ValueExpression::GlobalInit { global_idx } => {
                    if *global_idx > 10000 {
                        return Err(validation_error!(
                            "Global index {} seems unreasonably large",
                            global_idx
                        ));
                    }
                },
                ValueExpression::FunctionCall { func_idx, args } => {
                    if *func_idx > 10000 {
                        return Err(validation_error!(
                            "Function index {} seems unreasonably large",
                            func_idx
                        ));
                    }

                    if args.len() > 1000 {
                        return Err(validation_error!(
                            "Function call has too many arguments ({})",
                            args.len()
                        ));
                    }
                },
                ValueExpression::Const(_) => {
                    // Constants are validated elsewhere
                },
            }
        }

        Ok(())
    }
}

/// Resource operation in a canonical function
#[derive(Debug, Clone)]
pub enum FormatResourceOperation {
    /// New resource operation
    New(ResourceNew),
    /// Drop a resource
    Drop(ResourceDrop),
    /// Resource representation operation
    Rep(ResourceRep),
}
