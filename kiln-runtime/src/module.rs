// Module implementation for runtime execution
//
// This module provides the core runtime implementation of WebAssembly modules
// used by the runtime execution engine.

// Use alloc (re-exported through lib.rs)
use alloc::{
    format,
    vec::Vec,
};

// Import tracing utilities
#[cfg(feature = "tracing")]
use kiln_foundation::tracing::{debug, trace, warn, ModuleTrace, ImportTrace};

use kiln_foundation::MemoryProvider;
use kiln_format::{
    module::{
        ExportKind as FormatExportKind,
        ImportDesc as FormatImportDesc,
    },
    DataSegment as KilnDataSegment,
    ElementSegment as KilnElementSegment,
};
// Re-export for module_builder
pub use kiln_foundation::types::LocalEntry;
use kiln_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    types::{
        CustomSection as KilnCustomSection,
        DataMode as KilnDataMode,
        ElementMode as KilnElementMode,
        ExportDesc as KilnExportDesc,
        FuncType as KilnFuncType,
        GlobalType as KilnGlobalType,
        ImportDesc as KilnImportDesc,
        Limits as KilnLimits,
        LocalEntry as KilnLocalEntry,
        MemoryType as KilnMemoryType,
        RefType as KilnRefType,
        TableType as KilnTableType,
        ValueType as KilnValueType,
        ValueType, // Also import without alias
    },
    values::{
        Value as KilnValue,
        Value,
    }, // Also import without alias
};

use crate::prelude::CoreMemoryType;

// Type alias for the runtime ImportDesc
pub type RuntimeImportDesc = KilnImportDesc<RuntimeProvider>;

// HashMap is not needed with clean architecture using BoundedMap
use kiln_foundation::{
    bounded_collections::BoundedMap,
    traits::{
        BoundedCapacity,
        Checksummable,
        FromBytes,
        ToBytes,
    },
};

// Unified memory allocation using safe_managed_alloc! - NO hardcoded providers
// All memory allocation goes through safe_managed_alloc!(size, crate_id) as per CLAUDE.md

// Use the unified RuntimeProvider from bounded_runtime_infra
use crate::bounded_runtime_infra::{
    create_runtime_provider,
    RuntimeProvider,
};
use crate::{
    global::Global,
    memory::Memory,
    prelude::{
        RuntimeString,
        ToString,
        *,
    },
    table::Table,
};
// ImportMap: stores imports per namespace
// 128 max imports per namespace (increased from 32 for ML inference modules)
type ImportMap = BoundedMap<
    kiln_foundation::bounded::BoundedString<256>,
    Import,
    128,
    RuntimeProvider,
>;
// ModuleImports: maps namespace to ImportMap
// 256 max namespaces (increased for large component models)
type ModuleImports = BoundedMap<
    kiln_foundation::bounded::BoundedString<256>,
    ImportMap,
    256,
    RuntimeProvider,
>;
type CustomSections = BoundedMap<
    kiln_foundation::bounded::BoundedString<256>,
    kiln_foundation::bounded::BoundedVec<u8, 4096, RuntimeProvider>,
    16,
    RuntimeProvider,
>;
type ExportMap = kiln_foundation::direct_map::DirectMap<
    kiln_foundation::bounded::BoundedString<256>,
    Export,
    256, // Increased from 64 to handle TinyGo modules with many exports
>;

// Additional type aliases for struct fields to use unified RuntimeProvider
type BoundedExportName = kiln_foundation::bounded::BoundedString<128>;
type BoundedImportName = kiln_foundation::bounded::BoundedString<128>;
type BoundedModuleName = kiln_foundation::bounded::BoundedString<128>;
type BoundedLocalsVec = kiln_foundation::bounded::BoundedVec<KilnLocalEntry, 256, RuntimeProvider>;
type BoundedElementItems = kiln_foundation::bounded::BoundedVec<u32, 4096, RuntimeProvider>;
// Data init storage: Vec for large segments
type BoundedDataInit = Vec<u8>;
// Module types: 512 for ML modules with many tensor operation signatures
type BoundedModuleTypes =
    kiln_foundation::bounded::BoundedVec<KilnFuncType, 512, RuntimeProvider>;
// Function limit: 8192 for large ML/inference modules
type BoundedFunctionVec = kiln_foundation::bounded::BoundedVec<Function, 8192, RuntimeProvider>;
type BoundedTableVec = kiln_foundation::bounded::BoundedVec<TableWrapper, 64, RuntimeProvider>;
type BoundedMemoryVec = kiln_foundation::bounded::BoundedVec<MemoryWrapper, 64, RuntimeProvider>;
// Globals limit: 512 for modules with many constants
type BoundedGlobalVec = kiln_foundation::bounded::BoundedVec<GlobalWrapper, 512, RuntimeProvider>;
// Elements limit: 512 for indirect function tables
type BoundedElementVec = kiln_foundation::bounded::BoundedVec<Element, 512, RuntimeProvider>;
// Data segments limit: 512 for modules with embedded data
type BoundedDataVec = kiln_foundation::bounded::BoundedVec<Data, 512, RuntimeProvider>;

// Binary storage: Vec for large modules
type BoundedBinary = Vec<u8>;

/// Convert MemoryType to CoreMemoryType
fn to_core_memory_type(memory_type: KilnMemoryType) -> CoreMemoryType {
    CoreMemoryType {
        limits: memory_type.limits,
        shared: memory_type.shared,
        memory64: memory_type.memory64,
        page_size: memory_type.page_size,
    }
}

/// A WebAssembly expression (sequence of instructions)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KilnExpr {
    /// Parsed instructions (simplified representation)
    pub instructions: Vec<kiln_foundation::types::Instruction<RuntimeProvider>>,
}

impl KilnExpr {
    /// Returns the length of the instruction sequence
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Returns true if the expression is empty
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }
}

/// Represents a WebAssembly export kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportKind {
    /// Function export
    #[default]
    Function,
    /// Table export
    Table,
    /// Memory export
    Memory,
    /// Global export
    Global,
    /// Tag export (exception handling)
    Tag,
}

/// Represents an export in a WebAssembly module
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Export {
    /// Export name
    pub name:  BoundedExportName,
    /// Export kind
    pub kind:  ExportKind,
    /// Export index
    pub index: u32,
}

impl Export {
    /// Creates a new export
    pub fn new(name: &str, kind: ExportKind, index: u32) -> Result<Self> {
        let bounded_name =
            kiln_foundation::bounded::BoundedString::from_str_truncate(name)?;
        Ok(Self {
            name: bounded_name,
            kind,
            index,
        })
    }
}

impl kiln_foundation::traits::Checksummable for Export {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        self.name.update_checksum(checksum);
        checksum.update_slice(&(self.kind as u8).to_le_bytes());
        checksum.update_slice(&self.index.to_le_bytes());
    }
}

impl kiln_foundation::traits::ToBytes for Export {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + 1 + 4 // name + kind (1 byte) + index (4
                                            // bytes)
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'_>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        writer.write_all(&(self.kind as u8).to_le_bytes())?;
        writer.write_all(&self.index.to_le_bytes())
    }
}

impl kiln_foundation::traits::FromBytes for Export {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'_>,
        provider: &P,
    ) -> Result<Self> {
        let name =
            kiln_foundation::bounded::BoundedString::from_bytes_with_provider(reader, provider)?;

        let mut kind_bytes = [0u8; 1];
        reader.read_exact(&mut kind_bytes)?;
        let kind = match kind_bytes[0] {
            0 => ExportKind::Function,
            1 => ExportKind::Table,
            2 => ExportKind::Memory,
            3 => ExportKind::Global,
            _ => ExportKind::Function, // Default fallback
        };

        let mut index_bytes = [0u8; 4];
        reader.read_exact(&mut index_bytes)?;
        let index = u32::from_le_bytes(index_bytes);

        Ok(Self { name, kind, index })
    }
}

/// Represents an import in a WebAssembly module
#[derive(Debug, Clone)]
pub struct Import {
    /// Module name
    pub module: BoundedImportName,
    /// Import name
    pub name:   BoundedImportName,
    /// Import type
    pub ty:     ExternType<RuntimeProvider>,
    /// Import description
    pub desc:   RuntimeImportDesc,
}

impl Import {
    /// Creates a new import
    pub fn new(
        module: &str,
        name: &str,
        ty: ExternType<RuntimeProvider>,
        desc: RuntimeImportDesc,
    ) -> Result<Self> {
        let bounded_module =
            kiln_foundation::bounded::BoundedString::from_str_truncate(module)?;
        let bounded_name =
            kiln_foundation::bounded::BoundedString::from_str_truncate(name)?;
        Ok(Self {
            module: bounded_module,
            name: bounded_name,
            ty,
            desc,
        })
    }
}

impl Default for Import {
    fn default() -> Self {
        Self {
            module: kiln_foundation::bounded::BoundedString::from_str_truncate("")
                .expect("Failed to create empty BoundedString for Import::default()"),
            name:   kiln_foundation::bounded::BoundedString::from_str_truncate("")
                .expect("Failed to create empty BoundedString for Import::default()"),
            ty:     ExternType::default(),
            desc:   RuntimeImportDesc::Function(0),
        }
    }
}

impl PartialEq for Import {
    fn eq(&self, other: &Self) -> bool {
        self.module == other.module && self.name == other.name
    }
}

impl Eq for Import {}

impl kiln_foundation::traits::Checksummable for Import {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        self.module.update_checksum(checksum);
        self.name.update_checksum(checksum);
    }
}

impl kiln_foundation::traits::ToBytes for Import {
    fn serialized_size(&self) -> usize {
        self.module.serialized_size() + self.name.serialized_size() + 4 // simplified
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'_>,
        provider: &P,
    ) -> Result<()> {
        self.module.to_bytes_with_provider(writer, provider)?;
        self.name.to_bytes_with_provider(writer, provider)
    }
}

impl kiln_foundation::traits::FromBytes for Import {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'_>,
        provider: &P,
    ) -> Result<Self> {
        let module =
            kiln_foundation::bounded::BoundedString::from_bytes_with_provider(reader, provider)?;
        let name =
            kiln_foundation::bounded::BoundedString::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            module,
            name,
            ty: ExternType::default(), // simplified
            desc: RuntimeImportDesc::Function(0),
        })
    }
}

/// Represents a WebAssembly function in the runtime
#[derive(Debug, Clone)]
pub struct Function {
    /// The type index of the function (referring to Module.types)
    pub type_idx: u32,
    /// The parsed local variable declarations
    pub locals:   BoundedLocalsVec,
    /// The parsed instructions that make up the function body
    pub body:     KilnExpr,
}

impl Default for Function {
    fn default() -> Self {
        let provider = create_runtime_provider()
            .expect("Failed to create runtime provider for Function::default()");
        Self {
            type_idx: 0,
            locals:   BoundedLocalsVec::new(provider)
                .expect("Failed to create locals vector for Function::default()"),
            body:     KilnExpr::default(),
        }
    }
}

impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        self.type_idx == other.type_idx
    }
}

impl Eq for Function {}

impl kiln_foundation::traits::Checksummable for Function {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        checksum.update_slice(&self.type_idx.to_le_bytes());
    }
}

impl kiln_foundation::traits::ToBytes for Function {
    fn serialized_size(&self) -> usize {
        8 // simplified
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&self.type_idx.to_le_bytes())
    }
}

impl kiln_foundation::traits::FromBytes for Function {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 4];
        reader.read_exact(&mut bytes)?;
        let type_idx = u32::from_le_bytes(bytes);
        let provider = create_runtime_provider().map_err(|_| {
            kiln_error::Error::memory_error("Failed to allocate provider for function locals")
        })?;
        Ok(Self {
            type_idx,
            locals: BoundedLocalsVec::new(provider)?,
            body: KilnExpr::default(),
        })
    }
}

/// Represents the value of an export
#[derive(Debug, Clone)]
pub enum ExportItem {
    /// A function with the specified index
    Function(u32),
    /// A table with the specified index
    Table(TableWrapper),
    /// A memory with the specified index
    Memory(MemoryWrapper),
    /// A global with the specified index
    Global(GlobalWrapper),
}

/// Represents an element segment for tables in the runtime
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    /// Element segment mode (active, passive, or declarative)
    pub mode:         KilnElementMode,
    /// Index of the target table (for active elements)
    pub table_idx:    Option<u32>,
    /// Offset expression for element placement
    pub offset_expr:  Option<KilnExpr>,
    /// Type of elements in this segment
    pub element_type: KilnRefType,
    /// Element items (function indices or expressions)
    pub items:        BoundedElementItems,
    /// Deferred item expressions that need global evaluation (e.g., global.get $gf)
    pub item_exprs:   Vec<(u32, KilnExpr)>, // (item_index, expression)
}

impl kiln_foundation::traits::Checksummable for Element {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        let mode_byte = match &self.mode {
            KilnElementMode::Active { .. } => 0u8,
            KilnElementMode::Passive => 1u8,
            KilnElementMode::Declarative => 2u8,
        };
        checksum.update_slice(&mode_byte.to_le_bytes());
        if let Some(table_idx) = self.table_idx {
            checksum.update_slice(&table_idx.to_le_bytes());
        }
    }
}

impl kiln_foundation::traits::ToBytes for Element {
    fn serialized_size(&self) -> usize {
        // 1 (mode) + 4 (table_index) + 4 (offset) + 4 (items count) + items.len() * 4
        1 + 4 + 4 + 4 + self.items.len() * 4
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        // Serialize mode with offset
        let (mode_byte, table_idx, offset) = match &self.mode {
            KilnElementMode::Active { table_index, offset } => (0u8, *table_index, *offset),
            KilnElementMode::Passive => (1u8, 0, 0),
            KilnElementMode::Declarative => (2u8, 0, 0),
        };
        writer.write_all(&mode_byte.to_le_bytes())?;
        writer.write_all(&table_idx.to_le_bytes())?;
        writer.write_all(&offset.to_le_bytes())?;

        // Serialize items count and items
        let items_count = self.items.len() as u32;
        writer.write_all(&items_count.to_le_bytes())?;
        for i in 0..self.items.len() {
            if let Ok(item) = self.items.get(i) {
                writer.write_all(&item.to_le_bytes())?;
            }
        }
        Ok(())
    }
}

impl kiln_foundation::traits::FromBytes for Element {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        // Read mode byte
        let mut mode_byte = [0u8; 1];
        reader.read_exact(&mut mode_byte)?;

        // Read table_index
        let mut table_idx_bytes = [0u8; 4];
        reader.read_exact(&mut table_idx_bytes)?;
        let table_index = u32::from_le_bytes(table_idx_bytes);

        // Read offset
        let mut offset_bytes = [0u8; 4];
        reader.read_exact(&mut offset_bytes)?;
        let offset = u32::from_le_bytes(offset_bytes);

        let mode = match mode_byte[0] {
            0 => KilnElementMode::Active {
                table_index,
                offset,
            },
            1 => KilnElementMode::Passive,
            _ => KilnElementMode::Declarative,
        };

        // Read items count
        let mut count_bytes = [0u8; 4];
        reader.read_exact(&mut count_bytes)?;
        let items_count = u32::from_le_bytes(count_bytes) as usize;

        // Read items
        let provider = create_runtime_provider()?;
        let mut items = BoundedElementItems::new(provider)?;
        for _ in 0..items_count {
            let mut item_bytes = [0u8; 4];
            reader.read_exact(&mut item_bytes)?;
            let item = u32::from_le_bytes(item_bytes);
            items.push(item)?;
        }

        Ok(Self {
            mode,
            table_idx: if table_index > 0 || mode_byte[0] == 0 { Some(table_index) } else { None },
            offset_expr: None,
            element_type: KilnRefType::Funcref,
            items,
            item_exprs: Vec::new(),
        })
    }
}

/// Represents a data segment for memories in the runtime
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Data {
    /// Data segment mode (active or passive)
    pub mode:        KilnDataMode,
    /// Index of the target memory (for active data)
    pub memory_idx:  Option<u32>,
    /// Offset expression for data placement
    pub offset_expr: Option<KilnExpr>,
    /// Initialization data bytes
    pub init:        BoundedDataInit,
}

impl kiln_foundation::traits::Checksummable for Data {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        let mode_byte = match &self.mode {
            KilnDataMode::Active { .. } => 0u8,
            KilnDataMode::Passive => 1u8,
        };
        checksum.update_slice(&mode_byte.to_le_bytes());
        if let Some(memory_idx) = self.memory_idx {
            checksum.update_slice(&memory_idx.to_le_bytes());
        }
        checksum.update_slice(&(self.init.len() as u32).to_le_bytes());
    }
}

impl kiln_foundation::traits::ToBytes for Data {
    fn serialized_size(&self) -> usize {
        16 + self.init.len() // simplified
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        let mode_byte = match &self.mode {
            KilnDataMode::Active { .. } => 0u8,
            KilnDataMode::Passive => 1u8,
        };
        writer.write_all(&mode_byte.to_le_bytes())?;
        writer.write_all(&self.memory_idx.unwrap_or(0).to_le_bytes())?;
        writer.write_all(&(self.init.len() as u32).to_le_bytes())
    }
}

impl kiln_foundation::traits::FromBytes for Data {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 1];
        reader.read_exact(&mut bytes)?;
        let mode = match bytes[0] {
            0 => KilnDataMode::Active {
                memory_index: 0,
                offset:       0,
            },
            _ => KilnDataMode::Passive,
        };

        let mut idx_bytes = [0u8; 4];
        reader.read_exact(&mut idx_bytes)?;
        let memory_idx = Some(u32::from_le_bytes(idx_bytes));

        reader.read_exact(&mut idx_bytes)?;
        let _len = u32::from_le_bytes(idx_bytes);

        let init = Vec::new();

        Ok(Self {
            mode,
            memory_idx,
            offset_expr: None,
            init,
        })
    }
}

impl Data {
    /// Returns a reference to the data in this segment
    pub fn data(&self) -> Result<&[u8]> {
        Ok(&self.init)
    }
}

/// Storage type for a GC field (packed or full value type)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GcFieldStorage {
    /// Full value type (identified by wasm byte encoding)
    Value(u8),
    /// Packed i8
    I8,
    /// Packed i16
    I16,
}

/// A single field in a GC struct type
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcField {
    /// The storage type of this field
    pub storage: GcFieldStorage,
    /// Whether this field is mutable
    pub mutable: bool,
}

/// GC composite type information for struct and array types
///
/// Stores the parsed field/element type information needed at runtime
/// for GC instructions like struct.new, array.new, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GcTypeInfo {
    /// This type index is a function type (no GC info needed)
    Func,
    /// Struct type with field definitions
    Struct(Vec<GcField>),
    /// Array type with element definition
    Array(GcField),
}

impl GcField {
    /// Get the size of this field's storage type in bytes
    pub fn size_in_bytes(&self) -> usize {
        match self.storage {
            GcFieldStorage::I8 => 1,
            GcFieldStorage::I16 => 2,
            GcFieldStorage::Value(byte) => match byte {
                0x7F => 4,  // i32
                0x7E => 8,  // i64
                0x7D => 4,  // f32
                0x7C => 8,  // f64
                0x7B => 16, // v128
                // Reference types are 4 bytes (ref index)
                _ => 4,
            },
        }
    }

    /// Get the default value for this field's storage type
    pub fn default_value(&self) -> Value {
        use kiln_foundation::values::{FloatBits32, FloatBits64, V128};
        match &self.storage {
            GcFieldStorage::I8 |
            GcFieldStorage::I16 |
            GcFieldStorage::Value(0x7F) => Value::I32(0),
            GcFieldStorage::Value(0x7E) => Value::I64(0),
            GcFieldStorage::Value(0x7D) => Value::F32(FloatBits32::from_f32(0.0)),
            GcFieldStorage::Value(0x7C) => Value::F64(FloatBits64::from_f64(0.0)),
            GcFieldStorage::Value(0x7B) => Value::V128(V128 { bytes: [0u8; 16] }),
            GcFieldStorage::Value(0x6C) => Value::I31Ref(None),
            GcFieldStorage::Value(0x6B) => Value::StructRef(None),
            GcFieldStorage::Value(0x6A) => Value::ArrayRef(None),
            GcFieldStorage::Value(0x70) => Value::FuncRef(None),
            GcFieldStorage::Value(0x6F) => Value::ExternRef(None),
            GcFieldStorage::Value(0x6E) => Value::StructRef(None),  // anyref
            GcFieldStorage::Value(0x6D) => Value::StructRef(None),  // eqref
            GcFieldStorage::Value(0x63) |
            GcFieldStorage::Value(0x64) => Value::FuncRef(None),    // ref null/non-null
            _ => Value::I32(0),
        }
    }
}

/// Represents a WebAssembly module in the runtime
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Module types (function signatures)
    pub types:           Vec<KilnFuncType>,
    /// Imported functions, tables, memories, and globals
    pub imports:         ModuleImports,
    /// Ordered list of imports for index-based lookup (module_name, field_name)
    pub import_order:    Vec<(String, String)>,
    /// Function definitions
    pub functions:       Vec<Function>,
    /// Table instances
    pub tables:          Vec<TableWrapper>,
    /// Memory instances
    pub memories:        Vec<MemoryWrapper>,
    /// Global variable instances
    /// Uses Vec instead of BoundedVec because GC reference types (ArrayRef, StructRef)
    /// contain heap-allocated data that cannot survive BoundedVec serialization round-trips.
    pub globals:         Vec<GlobalWrapper>,
    /// Exception tag definitions (exception handling proposal)
    pub tags:            Vec<kiln_foundation::types::TagType>,
    /// Element segments for tables
    pub elements:        Vec<Element>,
    /// Pre-resolved element item values for each element segment.
    /// Indexed by [segment_idx][item_idx]. Contains the evaluated Value for
    /// each element item (including those using ref.i31, struct.new, etc.).
    /// Used by table.init to copy GC-typed elements from passive segments.
    pub resolved_elem_items: Vec<Vec<kiln_foundation::values::Value>>,
    /// Data segments for memories
    pub data:            Vec<Data>,
    /// Start function index
    pub start:           Option<u32>,
    /// Custom sections
    pub custom_sections: CustomSections,
    /// Exports (functions, tables, memories, and globals)
    pub exports:         ExportMap,
    /// Optional name for the module
    pub name:            Option<BoundedModuleName>,
    /// Original binary (if available)
    pub binary:          Option<BoundedBinary>,
    /// Execution validation flag
    pub validated:       bool,
    /// Number of global imports (for proper global indexing)
    pub num_global_imports: usize,
    /// Types of imported globals (for creating placeholders during instantiation)
    pub global_import_types: Vec<kiln_foundation::types::GlobalType>,
    /// Raw init expression bytes for defined globals (for deferred evaluation)
    /// Stored as (global_type, init_bytes) for each defined global
    pub deferred_global_inits: Vec<(kiln_foundation::types::GlobalType, Vec<u8>)>,
    /// Types of imports in order (parallels import_order)
    /// This provides fast lookup for import kind detection during linking
    pub import_types: Vec<RuntimeImportDesc>,
    /// Number of imported functions (set during decoding/loading)
    /// Used by the engine to distinguish import calls from local calls
    pub num_import_functions: usize,
    /// GC type information indexed by type index
    /// Stores struct field info and array element info needed for GC instructions
    pub gc_types: Vec<GcTypeInfo>,
    /// Supertype index for each type (None if no supertype declared)
    /// Used by ref_test_value to walk the type hierarchy for subtype checking
    pub type_supertypes: Vec<Option<u32>>,
    /// Init expression bytes for tables with explicit init expressions.
    /// Indexed by table definition index (not including imported tables).
    /// None means the table uses the default null value for its element type.
    pub table_init_exprs: Vec<Option<Vec<u8>>>,
}

impl Module {
    /// Push memory
    pub fn push_memory(&mut self, memory: MemoryWrapper) -> Result<()> {
        self.memories.push(memory);
        Ok(())
    }

    /// Count the number of tag imports in the module
    pub fn count_tag_imports(&self) -> usize {
        self.import_types.iter()
            .filter(|desc| matches!(desc, RuntimeImportDesc::Tag(_)))
            .count()
    }

    /// Get the tag type for a given tag index (accounting for imports)
    /// Tag indices include both imported and defined tags:
    /// - Indices 0..N-1 are imported tags
    /// - Indices N+ are defined tags
    pub fn get_tag_type(&self, tag_idx: u32) -> Option<&kiln_foundation::types::TagType> {
        let num_tag_imports = self.count_tag_imports();
        if (tag_idx as usize) < num_tag_imports {
            // This is an imported tag - find it in import_types
            let mut import_idx = 0;
            for desc in &self.import_types {
                if let RuntimeImportDesc::Tag(tag_type) = desc {
                    if import_idx == tag_idx as usize {
                        return Some(tag_type);
                    }
                    import_idx += 1;
                }
            }
            None
        } else {
            // This is a defined tag
            let defined_idx = tag_idx as usize - num_tag_imports;
            self.tags.get(defined_idx)
        }
    }

    /// Evaluate a constant expression and return its value.
    /// Supports both simple const expressions and extended const expressions (WebAssembly 2.0).
    /// Extended const expressions allow i32/i64 add, sub, mul operations.
    fn evaluate_const_expr(
        init_bytes: &[u8],
        num_global_imports: usize,
        global_import_types: &[kiln_foundation::types::GlobalType],
        defined_globals: &[GlobalWrapper],
        current_global_idx: usize,
        gc_types: &[GcTypeInfo],
    ) -> Result<kiln_foundation::values::Value> {
        use kiln_foundation::values::{Value, FloatBits32, FloatBits64, GcArrayRef, GcStructRef};

        let mut stack: Vec<Value> = Vec::new();
        let mut pos = 0;

        while pos < init_bytes.len() {
            let opcode = init_bytes[pos];
            pos += 1;

            match opcode {
                // end - return top of stack
                0x0B => {
                    return stack.pop().ok_or_else(|| Error::parse_error(
                        "Empty stack at end of constant expression"
                    ));
                }
                // i32.const
                0x41 => {
                    let (value, consumed) = crate::instruction_parser::read_leb128_i32(init_bytes, pos)?;
                    pos += consumed;
                    stack.push(Value::I32(value));
                }
                // i64.const
                0x42 => {
                    let (value, consumed) = crate::instruction_parser::read_leb128_i64(init_bytes, pos)?;
                    pos += consumed;
                    stack.push(Value::I64(value));
                }
                // f32.const
                0x43 => {
                    if pos + 4 > init_bytes.len() {
                        return Err(Error::parse_error("Truncated f32.const"));
                    }
                    let mut bytes = [0u8; 4];
                    bytes.copy_from_slice(&init_bytes[pos..pos + 4]);
                    pos += 4;
                    stack.push(Value::F32(FloatBits32(u32::from_le_bytes(bytes))));
                }
                // f64.const
                0x44 => {
                    if pos + 8 > init_bytes.len() {
                        return Err(Error::parse_error("Truncated f64.const"));
                    }
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&init_bytes[pos..pos + 8]);
                    pos += 8;
                    stack.push(Value::F64(FloatBits64(u64::from_le_bytes(bytes))));
                }
                // global.get
                0x23 => {
                    let (ref_idx, consumed) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                    pos += consumed;
                    let ref_idx = ref_idx as usize;

                    if ref_idx < num_global_imports {
                        // Referenced global is an import - use placeholder with correct type
                        // (actual value will be linked later during instantiation)
                        if ref_idx < global_import_types.len() {
                            let global_type = &global_import_types[ref_idx];
                            let placeholder = match global_type.value_type {
                                kiln_foundation::types::ValueType::I32 => Value::I32(0),
                                kiln_foundation::types::ValueType::I64 => Value::I64(0),
                                kiln_foundation::types::ValueType::F32 => Value::F32(FloatBits32(0)),
                                kiln_foundation::types::ValueType::F64 => Value::F64(FloatBits64(0)),
                                kiln_foundation::types::ValueType::FuncRef => Value::FuncRef(None),
                                kiln_foundation::types::ValueType::ExternRef => Value::ExternRef(None),
                                kiln_foundation::types::ValueType::V128 => Value::V128(kiln_foundation::values::V128 { bytes: [0; 16] }),
                                // Unsupported types for now
                                _ => return Err(Error::not_supported_unsupported_operation(
                                    "Unsupported global import type for constant expression",
                                )),
                            };
                            stack.push(placeholder);
                        } else {
                            return Err(Error::validation_error("global.get references unknown import"));
                        }
                    } else {
                        let defined_idx = ref_idx - num_global_imports;
                        if defined_idx < current_global_idx && defined_idx < defined_globals.len() {
                            match defined_globals.get(defined_idx) {
                                Some(ref_global) => {
                                    let value = ref_global.get()?;
                                    stack.push(value);
                                },
                                None => return Err(Error::validation_error("global.get references non-existent global")),
                            }
                        } else {
                            return Err(Error::validation_error("global.get forward reference"));
                        }
                    }
                }
                // ref.null
                0xD0 => {
                    // Parse heap type as s33 LEB128 - this handles both:
                    // - Abstract heap types (negative values: 0x70=func, 0x6F=extern, etc.)
                    // - Concrete type indices (non-negative values: 0, 1, 2, ...)
                    let (heap_type_s33, consumed) = crate::instruction_parser::read_leb128_i64(init_bytes, pos)?;
                    pos += consumed;

                    if heap_type_s33 >= 0 {
                        // Concrete type index - null reference to a specific type
                        // For function types, use FuncRef; for struct/array types, use appropriate ref
                        // For now, use FuncRef as the default for concrete type references
                        stack.push(Value::FuncRef(None));
                    } else {
                        // Abstract heap type - convert negative s33 to heap type byte
                        // In signed LEB128, single-byte values 0x6E-0x7F encode negative values:
                        // 0x6E (110) = -18, 0x6F (111) = -17, 0x70 (112) = -16, etc.
                        // To get back the byte: byte = s33 + 128 (for 7-bit signed)
                        let heap_type_byte = ((heap_type_s33 + 128) & 0x7F) as u8;
                        match heap_type_byte {
                            // Standard reference types
                            0x70 => stack.push(Value::FuncRef(None)),      // func
                            0x6F => stack.push(Value::ExternRef(None)),    // extern
                            // GC abstract heap types (using their Value representations)
                            0x6E => stack.push(Value::ExternRef(None)),    // any (anyref) - uses externref repr
                            0x6D => stack.push(Value::I31Ref(None)),       // eq (eqref) - uses i31ref repr
                            0x6C => stack.push(Value::I31Ref(None)),       // i31 (i31ref)
                            0x6B => stack.push(Value::StructRef(None)),    // struct (structref)
                            0x6A => stack.push(Value::ArrayRef(None)),     // array (arrayref)
                            0x69 => stack.push(Value::ExnRef(None)),       // exn (exnref)
                            // Bottom types (uninhabited, but valid null references)
                            0x73 => stack.push(Value::FuncRef(None)),      // nofunc (bottom for func)
                            0x72 => stack.push(Value::ExternRef(None)),    // noextern (bottom for extern)
                            0x71 => stack.push(Value::ExternRef(None)),    // none (bottom for any) - uses externref repr
                            0x74 => stack.push(Value::ExnRef(None)),       // noexn (bottom for exn)
                            _ => return Err(Error::parse_error("Unknown heap type in ref.null")),
                        }
                    }
                }
                // ref.func
                0xD2 => {
                    let (func_idx, consumed) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                    pos += consumed;
                    // ref.func creates a FuncRef with the function index
                    stack.push(Value::FuncRef(Some(kiln_foundation::values::FuncRef::from_index(func_idx))));
                }
                // i32.add
                0x6A => {
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.add"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.add"))?;
                    match (a, b) {
                        (Value::I32(a), Value::I32(b)) => stack.push(Value::I32(a.wrapping_add(b))),
                        _ => return Err(Error::parse_error("Type mismatch in i32.add")),
                    }
                }
                // i32.sub
                0x6B => {
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.sub"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.sub"))?;
                    match (a, b) {
                        (Value::I32(a), Value::I32(b)) => stack.push(Value::I32(a.wrapping_sub(b))),
                        _ => return Err(Error::parse_error("Type mismatch in i32.sub")),
                    }
                }
                // i32.mul
                0x6C => {
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.mul"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.mul"))?;
                    match (a, b) {
                        (Value::I32(a), Value::I32(b)) => stack.push(Value::I32(a.wrapping_mul(b))),
                        _ => return Err(Error::parse_error("Type mismatch in i32.mul")),
                    }
                }
                // i64.add
                0x7C => {
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.add"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.add"))?;
                    match (a, b) {
                        (Value::I64(a), Value::I64(b)) => stack.push(Value::I64(a.wrapping_add(b))),
                        _ => return Err(Error::parse_error("Type mismatch in i64.add")),
                    }
                }
                // i64.sub
                0x7D => {
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.sub"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.sub"))?;
                    match (a, b) {
                        (Value::I64(a), Value::I64(b)) => stack.push(Value::I64(a.wrapping_sub(b))),
                        _ => return Err(Error::parse_error("Type mismatch in i64.sub")),
                    }
                }
                // i64.mul
                0x7E => {
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.mul"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.mul"))?;
                    match (a, b) {
                        (Value::I64(a), Value::I64(b)) => stack.push(Value::I64(a.wrapping_mul(b))),
                        _ => return Err(Error::parse_error("Type mismatch in i64.mul")),
                    }
                }
                // GC instructions (0xFB prefix) in constant expressions
                // WebAssembly 3.0 allows struct.new, array.new, ref.i31, etc. in const exprs
                0xFB => {
                    if pos >= init_bytes.len() {
                        return Err(Error::parse_error("Truncated GC prefix in constant expression"));
                    }
                    let (sub_opcode, consumed) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                    pos += consumed;
                    match sub_opcode {
                        // struct.new $t: pops field values, pushes structref
                        0x00 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            // Determine field count from gc_types
                            let field_count = match gc_types.get(type_idx as usize) {
                                Some(GcTypeInfo::Struct(fields)) => fields.len(),
                                _ => 0,
                            };
                            // Pop field values in reverse order (last field is on top)
                            let mut field_values = Vec::new();
                            for _ in 0..field_count {
                                let val = stack.pop().ok_or_else(|| Error::parse_error(
                                    "Stack underflow in struct.new const expr"
                                ))?;
                                field_values.push(val);
                            }
                            field_values.reverse();
                            let mut struct_ref = kiln_foundation::values::StructRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create struct in const expr"))?;
                            for val in field_values {
                                struct_ref.add_field(val).map_err(|_|
                                    Error::runtime_error("Failed to push field to struct in const expr"))?;
                            }
                            stack.push(Value::StructRef(Some(GcStructRef::new(struct_ref))));
                        }
                        // struct.new_default $t: pushes structref with default fields
                        0x01 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let mut struct_ref = kiln_foundation::values::StructRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create struct in const expr"))?;
                            if let Some(GcTypeInfo::Struct(fields)) = gc_types.get(type_idx as usize) {
                                for field in fields {
                                    struct_ref.add_field(field.default_value()).map_err(|_|
                                        Error::runtime_error("Failed to push default field to struct in const expr"))?;
                                }
                            }
                            stack.push(Value::StructRef(Some(GcStructRef::new(struct_ref))));
                        }
                        // array.new $t: [val i32] -> [(ref $t)]
                        0x06 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let length = match stack.pop() {
                                Some(Value::I32(n)) => n as u32,
                                _ => return Err(Error::parse_error("array.new const expr: expected i32 length")),
                            };
                            let init_value = stack.pop().ok_or_else(||
                                Error::parse_error("array.new const expr: expected init value"))?;
                            let mut array_ref = kiln_foundation::values::ArrayRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create array in const expr"))?;
                            for _ in 0..length {
                                array_ref.push(init_value.clone()).map_err(|_|
                                    Error::runtime_error("Failed to push to array in const expr"))?;
                            }
                            stack.push(Value::ArrayRef(Some(GcArrayRef::new(array_ref))));
                        }
                        // array.new_default $t: [i32] -> [(ref $t)]
                        0x07 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let length = match stack.pop() {
                                Some(Value::I32(n)) => n as u32,
                                _ => return Err(Error::parse_error("array.new_default const expr: expected i32 length")),
                            };
                            let default_val = match gc_types.get(type_idx as usize) {
                                Some(GcTypeInfo::Array(field)) => field.default_value(),
                                _ => Value::I32(0),
                            };
                            let mut array_ref = kiln_foundation::values::ArrayRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create array in const expr"))?;
                            for _ in 0..length {
                                array_ref.push(default_val.clone()).map_err(|_|
                                    Error::runtime_error("Failed to push to array in const expr"))?;
                            }
                            stack.push(Value::ArrayRef(Some(GcArrayRef::new(array_ref))));
                        }
                        // array.new_fixed $t $n: [val*n] -> [(ref $t)]
                        0x08 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let (count, consumed3) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed3;
                            let mut values = Vec::new();
                            for _ in 0..count {
                                let val = stack.pop().ok_or_else(||
                                    Error::parse_error("array.new_fixed const expr: stack underflow"))?;
                                values.push(val);
                            }
                            values.reverse();
                            let mut array_ref = kiln_foundation::values::ArrayRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create array in const expr"))?;
                            for val in values {
                                array_ref.push(val).map_err(|_|
                                    Error::runtime_error("Failed to push to array in const expr"))?;
                            }
                            stack.push(Value::ArrayRef(Some(GcArrayRef::new(array_ref))));
                        }
                        // ref.i31: [i32] -> [(ref i31)]
                        0x1C => {
                            let val = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in ref.i31"))?;
                            match val {
                                Value::I32(i) => {
                                    // i31ref stores the low 31 bits
                                    stack.push(Value::I31Ref(Some(i & 0x7FFF_FFFF)));
                                }
                                _ => return Err(Error::parse_error("Type mismatch in ref.i31: expected i32")),
                            }
                        }
                        // any.convert_extern: [(ref extern)] -> [(ref any)]
                        0x1A => {
                            let val = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in any.convert_extern"))?;
                            stack.push(val); // Type conversion (representation unchanged)
                        }
                        // extern.convert_any: [(ref any)] -> [(ref extern)]
                        0x1B => {
                            let val = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in extern.convert_any"))?;
                            stack.push(val); // Type conversion (representation unchanged)
                        }
                        _ => {
                            return Err(Error::parse_error("Unsupported GC opcode in constant expression"));
                        }
                    }
                }
                // SIMD instructions (0xFD prefix) in constant expressions
                0xFD => {
                    if pos >= init_bytes.len() {
                        return Err(Error::parse_error("Truncated SIMD prefix in constant expression"));
                    }
                    let (sub_opcode, consumed) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                    pos += consumed;
                    if sub_opcode == 12 {
                        // v128.const: 16 byte immediate
                        if pos + 16 > init_bytes.len() {
                            return Err(Error::parse_error("Truncated v128.const in constant expression"));
                        }
                        let mut bytes = [0u8; 16];
                        bytes.copy_from_slice(&init_bytes[pos..pos + 16]);
                        pos += 16;
                        stack.push(Value::V128(kiln_foundation::values::V128 { bytes }));
                    } else {
                        return Err(Error::parse_error("Unsupported SIMD opcode in constant expression"));
                    }
                }
                _ => {
                    return Err(Error::parse_error("Unknown opcode in constant expression"));
                }
            }
        }

        Err(Error::parse_error("Constant expression missing end opcode"))
    }

    /// Re-evaluate globals that depend on imported globals.
    /// This should be called after import values have been set in the instance globals.
    ///
    /// # Arguments
    /// * `instance_globals` - The instance's globals vector (with correct import values)
    ///
    /// # Returns
    /// A vector of (defined_global_idx, new_value) pairs for globals that were re-evaluated
    pub fn reevaluate_deferred_globals(
        &self,
        instance_globals: &[GlobalWrapper],
    ) -> Result<Vec<(usize, kiln_foundation::values::Value)>> {
        use kiln_foundation::values::Value;

        let mut updates = Vec::new();

        for (defined_idx, (global_type, init_bytes)) in self.deferred_global_inits.iter().enumerate() {
            // Check if this global's init expression contains global.get of an import
            // global.get opcode is 0x23, followed by the global index
            let mut needs_reevaluation = false;
            let mut pos = 0;
            while pos < init_bytes.len() {
                if init_bytes[pos] == 0x23 {
                    // global.get - check if it references an import
                    if pos + 1 < init_bytes.len() {
                        let (ref_idx, _) = crate::instruction_parser::read_leb128_u32(init_bytes, pos + 1)?;
                        if (ref_idx as usize) < self.num_global_imports {
                            needs_reevaluation = true;
                            break;
                        }
                    }
                }
                pos += 1;
            }

            if needs_reevaluation {
                // Re-evaluate this global using the instance's globals (which have correct import values)
                // Build a temporary globals vec for evaluation
                let eval_result = Self::evaluate_const_expr_with_instance_globals(
                    init_bytes,
                    self.num_global_imports,
                    instance_globals,
                    &self.gc_types,
                )?;

                let global_idx = self.num_global_imports + defined_idx;
                #[cfg(feature = "tracing")]
                trace!(global_idx = global_idx, value = ?eval_result, "Re-evaluated deferred global");
                updates.push((global_idx, eval_result));
            }
        }

        Ok(updates)
    }

    /// Evaluate a constant expression using instance globals (for deferred evaluation)
    pub(crate) fn evaluate_const_expr_with_instance_globals(
        init_bytes: &[u8],
        num_global_imports: usize,
        instance_globals: &[GlobalWrapper],
        gc_types: &[GcTypeInfo],
    ) -> Result<kiln_foundation::values::Value> {
        use kiln_foundation::values::{Value, FloatBits32, FloatBits64};

        let mut stack: Vec<Value> = Vec::new();
        let mut pos = 0;

        while pos < init_bytes.len() {
            let opcode = init_bytes[pos];
            pos += 1;

            match opcode {
                0x41 => {
                    // i32.const
                    let (value, consumed) = crate::instruction_parser::read_leb128_i32(init_bytes, pos)?;
                    pos += consumed;
                    stack.push(Value::I32(value));
                }
                0x42 => {
                    // i64.const
                    let (value, consumed) = crate::instruction_parser::read_leb128_i64(init_bytes, pos)?;
                    pos += consumed;
                    stack.push(Value::I64(value));
                }
                0x43 => {
                    // f32.const
                    if pos + 4 > init_bytes.len() {
                        return Err(Error::parse_error("Unexpected end of f32.const"));
                    }
                    let mut bytes = [0u8; 4];
                    bytes.copy_from_slice(&init_bytes[pos..pos + 4]);
                    pos += 4;
                    stack.push(Value::F32(FloatBits32(u32::from_le_bytes(bytes))));
                }
                0x44 => {
                    // f64.const
                    if pos + 8 > init_bytes.len() {
                        return Err(Error::parse_error("Unexpected end of f64.const"));
                    }
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&init_bytes[pos..pos + 8]);
                    pos += 8;
                    stack.push(Value::F64(FloatBits64(u64::from_le_bytes(bytes))));
                }
                0x23 => {
                    // global.get
                    let (ref_idx, consumed) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                    pos += consumed;

                    // Get value from instance globals
                    if (ref_idx as usize) < instance_globals.len() {
                        if let Ok(guard) = instance_globals[ref_idx as usize].0.read() {
                            stack.push(guard.get().clone());
                        } else {
                            return Err(Error::runtime_error("Failed to read global for deferred evaluation"));
                        }
                    } else {
                        return Err(Error::runtime_error("Global index out of bounds in deferred evaluation"));
                    }
                }
                0x6A => {
                    // i32.add
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.add"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.add"))?;
                    if let (Value::I32(va), Value::I32(vb)) = (a, b) {
                        stack.push(Value::I32(va.wrapping_add(vb)));
                    }
                }
                0x6B => {
                    // i32.sub
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.sub"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.sub"))?;
                    if let (Value::I32(va), Value::I32(vb)) = (a, b) {
                        stack.push(Value::I32(va.wrapping_sub(vb)));
                    }
                }
                0x6C => {
                    // i32.mul
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.mul"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i32.mul"))?;
                    if let (Value::I32(va), Value::I32(vb)) = (a, b) {
                        stack.push(Value::I32(va.wrapping_mul(vb)));
                    }
                }
                0x7C => {
                    // i64.add
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.add"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.add"))?;
                    if let (Value::I64(va), Value::I64(vb)) = (a, b) {
                        stack.push(Value::I64(va.wrapping_add(vb)));
                    }
                }
                0x7D => {
                    // i64.sub
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.sub"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.sub"))?;
                    if let (Value::I64(va), Value::I64(vb)) = (a, b) {
                        stack.push(Value::I64(va.wrapping_sub(vb)));
                    }
                }
                0x7E => {
                    // i64.mul
                    let b = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.mul"))?;
                    let a = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in i64.mul"))?;
                    if let (Value::I64(va), Value::I64(vb)) = (a, b) {
                        stack.push(Value::I64(va.wrapping_mul(vb)));
                    }
                }
                0x0B => {
                    // end - done
                    break;
                }
                0xD2 => {
                    // ref.func
                    let (func_idx, consumed) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                    pos += consumed;
                    stack.push(Value::FuncRef(Some(kiln_foundation::values::FuncRef::from_index(func_idx))));
                }
                0xD0 => {
                    // ref.null
                    if pos >= init_bytes.len() {
                        return Err(Error::parse_error("Unexpected end of ref.null"));
                    }
                    let (heap_type_s33, consumed) = crate::instruction_parser::read_leb128_i64(init_bytes, pos)?;
                    pos += consumed;
                    if heap_type_s33 >= 0 {
                        stack.push(Value::FuncRef(None));
                    } else {
                        let heap_type_byte = ((heap_type_s33 + 128) & 0x7F) as u8;
                        match heap_type_byte {
                            0x70 => stack.push(Value::FuncRef(None)),
                            0x6F => stack.push(Value::ExternRef(None)),
                            0x6E => stack.push(Value::ExternRef(None)),
                            0x6D => stack.push(Value::I31Ref(None)),
                            0x6C => stack.push(Value::I31Ref(None)),
                            0x6B => stack.push(Value::StructRef(None)),
                            0x6A => stack.push(Value::ArrayRef(None)),
                            0x69 => stack.push(Value::ExnRef(None)),
                            0x73 => stack.push(Value::FuncRef(None)),
                            0x72 => stack.push(Value::ExternRef(None)),
                            0x71 => stack.push(Value::ExternRef(None)),
                            0x74 => stack.push(Value::ExnRef(None)),
                            _ => stack.push(Value::FuncRef(None)),
                        }
                    }
                }
                // GC instructions (0xFB prefix) in constant expressions
                0xFB => {
                    use kiln_foundation::values::{GcArrayRef, GcStructRef};
                    if pos >= init_bytes.len() {
                        return Err(Error::parse_error("Truncated GC prefix in deferred const expr"));
                    }
                    let (sub_opcode, consumed) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                    pos += consumed;
                    match sub_opcode {
                        // struct.new $t
                        0x00 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let field_count = match gc_types.get(type_idx as usize) {
                                Some(GcTypeInfo::Struct(fields)) => fields.len(),
                                _ => 0,
                            };
                            let mut field_values = Vec::new();
                            for _ in 0..field_count {
                                let val = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in struct.new"))?;
                                field_values.push(val);
                            }
                            field_values.reverse();
                            let mut struct_ref = kiln_foundation::values::StructRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create struct"))?;
                            for val in field_values {
                                struct_ref.add_field(val).map_err(|_| Error::runtime_error("Failed to push struct field"))?;
                            }
                            stack.push(Value::StructRef(Some(GcStructRef::new(struct_ref))));
                        }
                        // struct.new_default $t
                        0x01 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let mut struct_ref = kiln_foundation::values::StructRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create struct"))?;
                            if let Some(GcTypeInfo::Struct(fields)) = gc_types.get(type_idx as usize) {
                                for field in fields {
                                    struct_ref.add_field(field.default_value()).map_err(|_|
                                        Error::runtime_error("Failed to push default struct field"))?;
                                }
                            }
                            stack.push(Value::StructRef(Some(GcStructRef::new(struct_ref))));
                        }
                        // array.new $t
                        0x06 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let length = match stack.pop() {
                                Some(Value::I32(n)) => n as u32,
                                _ => return Err(Error::parse_error("array.new: expected i32 length")),
                            };
                            let init_value = stack.pop().ok_or_else(||
                                Error::parse_error("array.new: expected init value"))?;
                            let mut array_ref = kiln_foundation::values::ArrayRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create array"))?;
                            for _ in 0..length {
                                array_ref.push(init_value.clone()).map_err(|_|
                                    Error::runtime_error("Failed to push to array"))?;
                            }
                            stack.push(Value::ArrayRef(Some(GcArrayRef::new(array_ref))));
                        }
                        // array.new_default $t
                        0x07 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let length = match stack.pop() {
                                Some(Value::I32(n)) => n as u32,
                                _ => return Err(Error::parse_error("array.new_default: expected i32 length")),
                            };
                            let default_val = match gc_types.get(type_idx as usize) {
                                Some(GcTypeInfo::Array(field)) => field.default_value(),
                                _ => Value::I32(0),
                            };
                            let mut array_ref = kiln_foundation::values::ArrayRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create array"))?;
                            for _ in 0..length {
                                array_ref.push(default_val.clone()).map_err(|_|
                                    Error::runtime_error("Failed to push to array"))?;
                            }
                            stack.push(Value::ArrayRef(Some(GcArrayRef::new(array_ref))));
                        }
                        // array.new_fixed $t $n
                        0x08 => {
                            let (type_idx, consumed2) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed2;
                            let (count, consumed3) = crate::instruction_parser::read_leb128_u32(init_bytes, pos)?;
                            pos += consumed3;
                            let mut values = Vec::new();
                            for _ in 0..count {
                                let val = stack.pop().ok_or_else(||
                                    Error::parse_error("array.new_fixed: stack underflow"))?;
                                values.push(val);
                            }
                            values.reverse();
                            let mut array_ref = kiln_foundation::values::ArrayRef::new(
                                type_idx,
                                kiln_foundation::traits::DefaultMemoryProvider::default()
                            ).map_err(|_| Error::runtime_error("Failed to create array"))?;
                            for val in values {
                                array_ref.push(val).map_err(|_|
                                    Error::runtime_error("Failed to push to array"))?;
                            }
                            stack.push(Value::ArrayRef(Some(GcArrayRef::new(array_ref))));
                        }
                        // ref.i31
                        0x1C => {
                            let val = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in ref.i31"))?;
                            match val {
                                Value::I32(i) => stack.push(Value::I31Ref(Some(i & 0x7FFF_FFFF))),
                                _ => return Err(Error::parse_error("Type mismatch in ref.i31")),
                            }
                        }
                        // any.convert_extern
                        0x1A => {
                            let val = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in any.convert_extern"))?;
                            stack.push(val);
                        }
                        // extern.convert_any
                        0x1B => {
                            let val = stack.pop().ok_or_else(|| Error::parse_error("Stack underflow in extern.convert_any"))?;
                            stack.push(val);
                        }
                        _ => {
                            return Err(Error::parse_error("Unsupported GC opcode in deferred const expr"));
                        }
                    }
                }
                _ => {
                    // Skip unknown opcodes for now
                }
            }
        }

        stack.pop().ok_or_else(|| Error::parse_error("Empty stack after deferred global evaluation"))
    }

    /// REMOVED: All Module::empty(), try_empty(), bootstrap_empty(), etc.
    /// These functions used the old NoStdProvider system which causes stack overflow
    /// Use Module::new_empty() or Module::from_kiln_module() with proper initialization instead

    /// Creates an empty module using the unified memory system (create_runtime_provider)
    /// This replaces the old Module::new() and Module::empty() functions
    pub fn new_empty() -> Result<Self> {
        let provider = crate::bounded_runtime_infra::create_runtime_provider()?;
        Ok(Self {
            types: Vec::new(),
            imports: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            import_order: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: Vec::new(),
            tags: Vec::new(),
            elements: Vec::new(),
            resolved_elem_items: Vec::new(),
            data: Vec::new(),
            start: None,
            custom_sections: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            exports: kiln_foundation::direct_map::DirectMap::new(),
            name: None,
            binary: None,
            validated: false,
            num_global_imports: 0,
            global_import_types: Vec::new(),
            deferred_global_inits: Vec::new(),
            import_types: Vec::new(),
            num_import_functions: 0,
            gc_types: Vec::new(),
            type_supertypes: Vec::new(),
            table_init_exprs: Vec::new(),
        })
    }

    /// Convert a GcStorageType from the format crate to the runtime GcFieldStorage.
    fn convert_gc_storage_type(st: &kiln_format::module::GcStorageType) -> GcFieldStorage {
        match st {
            kiln_format::module::GcStorageType::I8 => GcFieldStorage::I8,
            kiln_format::module::GcStorageType::I16 => GcFieldStorage::I16,
            kiln_format::module::GcStorageType::Value(b) => GcFieldStorage::Value(*b),
            // Reference types use 0x64/0x63 (ref/ref null) encoding byte for field storage
            kiln_format::module::GcStorageType::RefType(_) => GcFieldStorage::Value(0x64),
            kiln_format::module::GcStorageType::RefTypeNull(_) => GcFieldStorage::Value(0x63),
        }
    }

    /// Creates a runtime Module from a `kiln_format::module::Module`.
    /// This is the primary constructor after decoding.
    pub fn from_kiln_module(kiln_module: &kiln_format::module::Module) -> Result<Box<Self>> {
        // Ensure memory system is initialized before creating providers
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;

        // Use create_runtime_provider (wraps safe_managed_alloc with proper types)
        let shared_provider = crate::bounded_runtime_infra::create_runtime_provider()?;

        // Create initial empty module with proper providers
        let imports_map = kiln_foundation::bounded_collections::BoundedMap::new(shared_provider.clone())?;
        let custom_sections_map = kiln_foundation::bounded_collections::BoundedMap::new(shared_provider.clone())?;
        let mut runtime_module = Self {
            types: Vec::new(),
            imports: imports_map,
            import_order: Vec::new(), // Ordered list of imports for index-based lookup
            functions: Vec::new(),
            tables: Vec::new(), // Vec in std mode to avoid serialization issues with Arc<Table>
            memories: Vec::new(),
            globals: Vec::new(),
            tags: Vec::new(), // Exception tags (exception handling proposal)
            elements: Vec::new(), // Vec in std mode for variable-size Element items
            resolved_elem_items: Vec::new(), // Will be populated after element processing
            data: Vec::new(), // Vec in std mode for large data segments
            start: kiln_module.start,
            custom_sections: custom_sections_map,
            exports: kiln_foundation::direct_map::DirectMap::new(),
            name: None,
            binary: None,
            validated: false,
            num_global_imports: 0, // Will be updated when processing imports
            global_import_types: Vec::new(), // Will be populated when processing imports
            deferred_global_inits: Vec::new(), // Will be populated when processing globals
            import_types: Vec::new(), // Will be populated when processing imports
            num_import_functions: 0, // Will be set after processing imports
            gc_types: Vec::new(), // Will be populated from rec_groups
            type_supertypes: Vec::new(), // Will be populated from rec_groups
            table_init_exprs: Vec::new(), // Will be populated from table section
        };

        // Populate GC type info from rec_groups
        {
            use kiln_format::module::CompositeTypeKind;
            // First pass: determine size
            let total_types: usize = kiln_module.rec_groups.iter()
                .map(|rg| rg.types.len())
                .sum();
            runtime_module.gc_types.reserve(total_types);

            // Collect types ordered by type_index
            let mut gc_type_entries: Vec<(u32, GcTypeInfo)> = Vec::new();
            for rec_group in &kiln_module.rec_groups {
                for sub_type in &rec_group.types {
                    let info = match &sub_type.composite_kind {
                        CompositeTypeKind::Func => GcTypeInfo::Func,
                        CompositeTypeKind::Struct => GcTypeInfo::Func, // Legacy variant, treat as func
                        CompositeTypeKind::Array => GcTypeInfo::Func, // Legacy variant, treat as func
                        CompositeTypeKind::StructWithFields(fields) => {
                            let gc_fields: Vec<GcField> = fields.iter().map(|f| {
                                let storage = Self::convert_gc_storage_type(&f.storage_type);
                                GcField { storage, mutable: f.mutable }
                            }).collect();
                            GcTypeInfo::Struct(gc_fields)
                        }
                        CompositeTypeKind::ArrayWithElement(elem) => {
                            let storage = Self::convert_gc_storage_type(&elem.storage_type);
                            GcTypeInfo::Array(GcField { storage, mutable: elem.mutable })
                        }
                    };
                    gc_type_entries.push((sub_type.type_index, info));
                }
            }
            // Also collect supertype info
            let mut supertype_entries: Vec<(u32, Option<u32>)> = Vec::new();
            for rec_group in &kiln_module.rec_groups {
                for sub_type in &rec_group.types {
                    let supertype = sub_type.supertype_indices.first().copied();
                    supertype_entries.push((sub_type.type_index, supertype));
                }
            }

            // Sort by type index and fill both vectors
            gc_type_entries.sort_by_key(|(idx, _)| *idx);
            supertype_entries.sort_by_key(|(idx, _)| *idx);
            for (_, info) in gc_type_entries {
                runtime_module.gc_types.push(info);
            }
            for (_, supertype) in supertype_entries {
                runtime_module.type_supertypes.push(supertype);
            }
        }

        // Convert types
        #[cfg(feature = "tracing")]
        debug!(type_count = kiln_module.types.len(), "Converting types from kiln_module");

        for (i, func_type) in kiln_module.types.iter().enumerate() {
            #[cfg(feature = "tracing")]
            trace!(type_idx = i, params_len = func_type.params.len(), results_len = func_type.results.len(), "Converting type");

            let param_types: Vec<_> = func_type.params.to_vec();
            let result_types: Vec<_> = func_type.results.to_vec();

            let kiln_func_type = KilnFuncType::new(param_types, result_types)?;

            runtime_module.types.push(kiln_func_type);

            #[cfg(feature = "tracing")]
            trace!(type_idx = i, total_types = runtime_module.types.len(), "Pushed type");
        }

        #[cfg(feature = "tracing")]
        debug!(total_types = runtime_module.types.len(), "Done converting types");

        // Convert imports
        #[cfg(feature = "tracing")]
        let import_span = ImportTrace::registering("", "", kiln_module.imports.len()).entered();
        #[cfg(feature = "tracing")]
        debug!(import_count = kiln_module.imports.len(), data_count = kiln_module.data.len(), "Processing imports from kiln_module");

        use kiln_format::module::ImportDesc as FormatImportDesc;

        let mut global_import_count = 0usize;
        for import in &kiln_module.imports {
            let desc = match &import.desc {
                FormatImportDesc::Function(type_idx) => RuntimeImportDesc::Function(*type_idx),
                FormatImportDesc::Table(tt) => RuntimeImportDesc::Table(tt.clone()),
                FormatImportDesc::Memory(mt) => RuntimeImportDesc::Memory(*mt),
                FormatImportDesc::Global(gt) => {
                    global_import_count += 1;
                    let global_type = kiln_foundation::types::GlobalType {
                        value_type: gt.value_type,
                        mutable:    gt.mutable,
                    };
                    // Store the global type for direct access during instantiation
                    // This bypasses the broken nested BoundedMap serialization
                    runtime_module.global_import_types.push(global_type.clone());
                    RuntimeImportDesc::Global(global_type)
                },
                FormatImportDesc::Tag(type_idx) => {
                    // Handle Tag import - convert to appropriate runtime representation
                    RuntimeImportDesc::Tag(kiln_foundation::types::TagType {
                        attribute: 0, // Exception tag attribute
                        type_idx: *type_idx,
                    })
                },
            };

            // Convert string to BoundedString - need different sizes for different use cases
            // 128-char strings for Import struct fields
            let bounded_module_128 = kiln_foundation::bounded::BoundedString::from_str_truncate(
                &import.module
            )?;
            let bounded_name_128 =
                kiln_foundation::bounded::BoundedString::from_str_truncate(&import.name)?;

            // 256-char strings for map keys
            let bounded_module_256 = kiln_foundation::bounded::BoundedString::from_str_truncate(
                &import.module
            )?;
            let bounded_name_256 =
                kiln_foundation::bounded::BoundedString::from_str_truncate(&import.name)?;

            let import_entry = Import {
                module: bounded_module_128,
                name: bounded_name_128,
                ty: kiln_foundation::component::ExternType::default(),
                desc,
            };

            // Get or create inner map for this module
            let mut inner_map = match runtime_module.imports.get(&bounded_module_256)? {
                Some(existing) => existing,
                None => ImportMap::new(crate::bounded_runtime_infra::create_runtime_provider()?)?,
            };

            // Insert the import into the inner map
            inner_map.insert(bounded_name_256, import_entry)?;

            // Update the outer map
            runtime_module.imports.insert(bounded_module_256.clone(), inner_map)?;

            // Track import order for index-based lookup
            runtime_module.import_order.push((import.module.to_string(), import.name.to_string()));
            // Also store the import type for fast lookup during linking
            let import_desc = match &import.desc {
                FormatImportDesc::Function(type_idx) => RuntimeImportDesc::Function(*type_idx),
                FormatImportDesc::Table(tt) => RuntimeImportDesc::Table(tt.clone()),
                FormatImportDesc::Memory(mt) => RuntimeImportDesc::Memory(*mt),
                FormatImportDesc::Global(gt) => RuntimeImportDesc::Global(kiln_foundation::types::GlobalType {
                    value_type: gt.value_type,
                    mutable: gt.mutable,
                }),
                FormatImportDesc::Tag(type_idx) => RuntimeImportDesc::Tag(kiln_foundation::types::TagType {
                    attribute: 0,
                    type_idx: *type_idx,
                }),
            };
            runtime_module.import_types.push(import_desc);

            #[cfg(feature = "tracing")]
            trace!(module = %import.module, name = %import.name, "Added import");
        }

        #[cfg(feature = "tracing")]
        debug!(imports_len = runtime_module.imports.len(), num_global_imports = global_import_count, "After import loop");

        // Set the count of global imports for proper index space mapping
        runtime_module.num_global_imports = global_import_count;

        // Count function imports for the engine's import/local call distinction
        runtime_module.num_import_functions = kiln_module.imports.iter()
            .filter(|imp| matches!(imp.desc, FormatImportDesc::Function(_)))
            .count();

        // Convert functions
        #[cfg(feature = "tracing")]
        debug!(function_count = kiln_module.functions.len(), "Converting functions from kiln_module");
        for (func_idx, func) in kiln_module.functions.iter().enumerate() {
            #[cfg(feature = "tracing")]
            trace!(func_idx = func_idx, type_idx = func.type_idx, locals_len = func.locals.len(), code_len = func.code.len(), "Processing function");

            // Handle imported functions (they have no code, but still need to be in the function table)
            let (locals, body) = if func.code.is_empty() {
                #[cfg(feature = "tracing")]
                trace!(func_idx = func_idx, "Function is imported (no code) - creating stub entry");

                // Imported function: create with empty locals and empty body
                let empty_locals = crate::type_conversion::convert_locals_to_bounded_with_provider(&[], shared_provider.clone())?;
                // Create empty instruction vector directly (don't parse empty bytecode)
                let empty_instructions = Vec::new();
                (empty_locals, KilnExpr { instructions: empty_instructions })
            } else {
                // Local function: convert locals and parse code
                #[cfg(feature = "tracing")]
                trace!(func_idx = func_idx, "About to convert locals for function");
                let locals = crate::type_conversion::convert_locals_to_bounded_with_provider(&func.locals, shared_provider.clone())?;

                #[cfg(feature = "tracing")]
                trace!(func_idx = func_idx, code_len = func.code.len(), "About to parse instructions for function");

                // Debug: show the raw bytecode
                #[cfg(feature = "tracing")]
                if !func.code.is_empty() {
                    trace!(func_idx = func_idx, bytecode_preview = ?&func.code[..func.code.len().min(20)], "Raw bytecode for function");
                } else {
                    trace!(func_idx = func_idx, "Warning - Function has empty code");
                }

                let instructions = crate::instruction_parser::parse_instructions_with_provider(&func.code, shared_provider.clone())?;

                #[cfg(feature = "tracing")]
                trace!(func_idx = func_idx, instruction_count = instructions.len(), "Parsed instructions for function");

                (locals, KilnExpr { instructions })
            };

            #[cfg(feature = "tracing")]
            trace!(func_idx = func_idx, "About to create runtime function");
            let runtime_func = Function {
                type_idx: func.type_idx,
                locals,
                body,
            };
            // CRITICAL DEBUG: Test provider directly before using BoundedVec
            #[cfg(feature = "tracing")]
            {
                trace!(func_idx = func_idx, "Testing RuntimeProvider directly before BoundedVec usage");

                // Test 1: Check provider size
                trace!(provider_size = shared_provider.size(), "Provider size");

                // Test 2: Try basic write_data directly
                let mut test_provider = shared_provider.clone();
                match test_provider.write_data(0, &[42u8, 43u8, 44u8, 45u8]) {
                    Ok(()) => {
                        trace!("Provider write_data works directly");
                    },
                    Err(e) => {
                        warn!(error = ?e, "Provider write_data fails");
                        return Err(Error::foundation_bounded_capacity_exceeded("Provider write_data broken"));
                    }
                }

                // Test 3: Try verify_access
                match test_provider.verify_access(0, 8) {
                    Ok(()) => {
                        trace!("Provider verify_access works");
                    },
                    Err(e) => {
                        warn!(error = ?e, "Provider verify_access fails");
                        return Err(Error::foundation_bounded_capacity_exceeded("Provider verify_access broken"));
                    }
                }

                // Now try the function push
                trace!(func_idx = func_idx, "Now testing Function push");
            }
            runtime_module.push_function(runtime_func)?;
            #[cfg(feature = "tracing")]
            trace!(func_idx = func_idx, "Successfully pushed runtime function");
        }

        // Convert exports
        #[cfg(feature = "tracing")]
        debug!(export_count = kiln_module.exports.len(), "Converting exports from kiln_module");
        for export in &kiln_module.exports {
            #[cfg(feature = "tracing")]
            trace!(name = %export.name, kind = ?export.kind, index = export.index, "Processing export");

            // Create the export name with correct provider size (8192)
            let name = kiln_foundation::bounded::BoundedString::from_str_truncate(
                &export.name
            )?;

            #[cfg(feature = "tracing")]
            trace!("Created export name BoundedString");

            // Create key with correct type for ExportMap (BoundedString<256,
            // RuntimeProvider>)
            let map_key = kiln_foundation::bounded::BoundedString::from_str_truncate(
                &export.name
            )?;

            #[cfg(feature = "tracing")]
            trace!("Created map_key BoundedString");

            let kind = match export.kind {
                FormatExportKind::Function => ExportKind::Function,
                FormatExportKind::Table => ExportKind::Table,
                FormatExportKind::Memory => ExportKind::Memory,
                FormatExportKind::Global => ExportKind::Global,
                FormatExportKind::Tag => ExportKind::Tag,
            };

            let runtime_export = Export {
                name,
                kind,
                index: export.index,
            };

            #[cfg(feature = "tracing")]
            trace!("Created Export struct, about to insert into exports map");

            runtime_module.exports.insert(map_key, runtime_export).map_err(|e| {
                #[cfg(feature = "tracing")]
                warn!(error = ?e, "exports.insert failed");
                e
            })?;

            #[cfg(feature = "tracing")]
            trace!("Successfully inserted export into map");
        }

        // Convert tables - CRITICAL for call_indirect!
        #[cfg(feature = "tracing")]
        debug!(table_count = kiln_module.tables.len(), "Converting tables from kiln_module");
        for (idx, table_type) in kiln_module.tables.iter().enumerate() {
            #[cfg(feature = "tracing")]
            trace!(table_idx = idx, table_type = ?table_type, "Creating table");

            // Create runtime table from the table type
            #[cfg(feature = "tracing")]
            trace!(table_idx = idx, "Calling Table::new");
            let table = match Table::new(table_type.clone()) {
                Ok(t) => t,
                Err(e) => {
                    #[cfg(feature = "tracing")]
                    warn!(table_idx = idx, error = ?e, "Table::new failed");
                    return Err(e);
                }
            };
            #[cfg(feature = "tracing")]
            trace!(table_idx = idx, "Table::new succeeded, creating wrapper");
            let wrapper = TableWrapper::new(table);
            #[cfg(feature = "tracing")]
            trace!(table_idx = idx, "Pushing to runtime_module.tables");
            runtime_module.tables.push(wrapper);

            #[cfg(feature = "tracing")]
            trace!(table_idx = idx, total_tables = runtime_module.tables.len(), "Successfully added to runtime_module.tables");
        }
        #[cfg(feature = "tracing")]
        debug!(total_tables = runtime_module.tables.len(), "Tables converted");

        // Copy table init expressions from the format module
        runtime_module.table_init_exprs = kiln_module.table_init_exprs.clone();

        // Convert memories - NOW ENABLED (stack overflow fixed)
        #[cfg(feature = "tracing")]
        debug!(memory_count = kiln_module.memories.len(), "Converting memories from kiln_module");
        for (mem_idx, memory) in kiln_module.memories.iter().enumerate() {
            #[cfg(feature = "tracing")]
            trace!(mem_idx = mem_idx, "Converting memory type");

            let memory_type = to_core_memory_type(*memory);

            #[cfg(feature = "tracing")]
            trace!(mem_idx = mem_idx, min_pages = memory_type.limits.min, max_pages = ?memory_type.limits.max, "Module declares memory");

            #[cfg(feature = "tracing")]
            trace!(mem_idx = mem_idx, "About to call Memory::new()");

            let memory_instance = Memory::new(memory_type)?;

            #[cfg(feature = "tracing")]
            trace!(mem_idx = mem_idx, "Memory::new() succeeded, about to create MemoryWrapper");

            #[cfg(feature = "tracing")]
            trace!(mem_idx = mem_idx, "About to create MemoryWrapper from Box<Memory>");
            let wrapper = MemoryWrapper::new(memory_instance);

            #[cfg(feature = "tracing")]
            trace!(mem_idx = mem_idx, "MemoryWrapper created successfully, about to push to runtime_module.memories");

            runtime_module.push_memory(wrapper)?;
            #[cfg(feature = "tracing")]
            trace!(mem_idx = mem_idx, "push_memory completed");

            #[cfg(feature = "tracing")]
            trace!(mem_idx = mem_idx, "Successfully pushed to runtime_module.memories");
        }

        // Convert globals - NOW ENABLED (stack overflow fixed)
        #[cfg(feature = "tracing")]
        debug!(global_count = kiln_module.globals.len(), "Converting globals from kiln_module");
        for (global_idx, global) in kiln_module.globals.iter().enumerate() {
            #[cfg(feature = "tracing")]
            trace!(global_idx = global_idx, "Processing global");
            // Parse the init expression to get the actual initial value
            // The init expression is typically a simple constant instruction like i32.const
            let init_bytes = global.init.as_slice();

            // Debug output to see what's in the init expression
            #[cfg(feature = "tracing")]
            trace!(global_idx = global_idx, init_bytes = ?init_bytes, "Global init bytes");

            // Store init bytes for potential deferred evaluation
            // This is needed when globals use global.get of imported globals
            let global_type = kiln_foundation::types::GlobalType {
                value_type: global.global_type.value_type,
                mutable: global.global_type.mutable,
            };
            runtime_module.deferred_global_inits.push((global_type, init_bytes.to_vec()));

            let initial_value = if !init_bytes.is_empty() {
                // Evaluate the init expression using a stack-based evaluator
                // This supports both simple const expressions and extended const expressions (WebAssembly 2.0)
                Self::evaluate_const_expr(
                    init_bytes,
                    runtime_module.num_global_imports,
                    &runtime_module.global_import_types,
                    &runtime_module.globals,
                    global_idx,
                    &runtime_module.gc_types,
                )?
            } else {
                // No init expression - this is an error, globals must be initialized
                return Err(Error::parse_error(
                    "Global has no init expression"
                ))
            };
            #[cfg(feature = "tracing")]
            debug!(
                global_idx = global_idx,
                value_type = ?global.global_type.value_type,
                mutable = global.global_type.mutable,
                "Global from kiln_format"
            );
            let new_global = Global::new(
                global.global_type.value_type,
                global.global_type.mutable,
                initial_value,
            )?;
            runtime_module.globals.push(GlobalWrapper(Arc::new(RwLock::new(new_global))));
        }

        // Convert tags (exception handling proposal)
        #[cfg(feature = "tracing")]
        debug!(tag_count = kiln_module.tags.len(), "Converting tags from kiln_module");
        for tag_type in &kiln_module.tags {
            runtime_module.tags.push(tag_type.clone());
        }

        // Convert data segments - CRITICAL for memory initialization!
        #[cfg(feature = "tracing")]
        debug!(data_count = kiln_module.data.len(), "Converting data segments from kiln_module");
        for (data_idx, data_seg) in kiln_module.data.iter().enumerate() {
            #[cfg(feature = "tracing")]
            trace!(data_idx = data_idx, "Processing data segment");
            // Convert PureDataSegment to runtime Data
            use kiln_format::pure_format_types::PureDataMode;

            // Parse offset from the offset_expr_bytes
            let (mode, memory_idx, offset_expr) = match &data_seg.mode {
                PureDataMode::Active { memory_index, offset_expr_len } => {
                    // Parse the offset expression bytes
                    let offset_bytes = &data_seg.offset_expr_bytes;
                    let offset: u32 = if !offset_bytes.is_empty() && offset_bytes[0] == 0x41 {
                        // i32.const - parse LEB128 value
                        let (value, _) = crate::instruction_parser::read_leb128_i32(offset_bytes, 1)?;
                        value as u32
                    } else {
                        0
                    };
                    #[cfg(feature = "tracing")]
                    debug!(data_idx = data_idx, memory_index = memory_index, offset = offset, "Data segment is active");

                    // Also create the offset expression for the Data struct
                    let instructions = if !offset_bytes.is_empty() {
                        crate::instruction_parser::parse_instructions_with_provider(
                            offset_bytes.as_slice(),
                            shared_provider.clone()
                        )?
                    } else {
                        Vec::new().into()
                    };

                    (
                        KilnDataMode::Active {
                            memory_index: *memory_index,
                            offset,
                        },
                        Some(*memory_index),
                        Some(KilnExpr { instructions })
                    )
                },
                PureDataMode::Passive => {
                    #[cfg(feature = "tracing")]
                    debug!(data_idx = data_idx, "Data segment is passive");
                    (KilnDataMode::Passive, None, None)
                },
            };

            // Convert init data bytes
            let init_bytes = &data_seg.data_bytes;
            #[cfg(feature = "tracing")]
            debug!(data_idx = data_idx, init_bytes_len = init_bytes.len(), "Data segment init bytes");

            let init: Vec<u8> = init_bytes.to_vec();

            let runtime_data = Data {
                mode,
                memory_idx,
                offset_expr,
                init,
            };

            #[cfg(feature = "tracing")]
            trace!(data_idx = data_idx, current_len = runtime_module.data.len(), "Pushing data segment to runtime_module.data");
            runtime_module.data.push(runtime_data);
            #[cfg(feature = "tracing")]
            debug!(data_idx = data_idx, "Successfully converted data segment");
        }
        #[cfg(feature = "tracing")]
        debug!(total_data_segments = runtime_module.data.len(), "Data segment conversion complete");

        // Convert element segments - CRITICAL for call_indirect and table initialization!
        #[cfg(feature = "tracing")]
        debug!(element_count = kiln_module.elements.len(), "Converting element segments from kiln_module");

        for (elem_idx, elem_seg) in kiln_module.elements.iter().enumerate() {
            use kiln_format::pure_format_types::{PureElementInit, PureElementMode};

            // Parse offset from the offset_expr_bytes
            let (mode, table_idx, offset_value) = match &elem_seg.mode {
                PureElementMode::Active { table_index, offset_expr_len } => {
                    // Parse the offset expression bytes
                    let offset_bytes = &elem_seg.offset_expr_bytes;
                    let offset: u32 = if !offset_bytes.is_empty() && offset_bytes[0] == 0x41 {
                        // i32.const - parse LEB128 value
                        let (value, _) = crate::instruction_parser::read_leb128_i32(offset_bytes, 1)?;
                        value as u32
                    } else {
                        0
                    };
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, table_index = table_index, offset = offset, "Element segment is active");

                    (
                        KilnElementMode::Active {
                            table_index: *table_index,
                            offset,
                        },
                        Some(*table_index),
                        offset,
                    )
                },
                PureElementMode::Passive => {
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, "Element segment is passive");
                    (KilnElementMode::Passive, None, 0)
                },
                PureElementMode::Declared => {
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, "Element segment is declared");
                    (KilnElementMode::Declarative, None, 0)
                },
            };

            // Extract function indices from element init data
            let provider = crate::bounded_runtime_infra::create_runtime_provider()?;
            let mut items = BoundedElementItems::new(provider)?;
            let mut deferred_item_exprs: Vec<(u32, KilnExpr)> = Vec::new();

            match &elem_seg.init_data {
                PureElementInit::FunctionIndices(func_indices) => {
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, count = func_indices.len(), "Element segment has function indices");
                    for (i, func_idx) in func_indices.iter().enumerate() {
                        #[cfg(feature = "tracing")]
                        if i < 5 || i == func_indices.len() - 1 {
                            trace!(elem_offset = offset_value + i as u32, func_idx = func_idx, "Element item = func");
                        }
                        items.push(*func_idx)?;
                    }
                },
                PureElementInit::ExpressionBytes(expr_bytes) => {
                    // For expression bytes, we'd need to evaluate them
                    // Handle ref.func and global.get instructions
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, count = expr_bytes.len(), "Element segment has expression items");
                    for (i, expr) in expr_bytes.iter().enumerate() {
                        if expr.is_empty() {
                            continue;
                        }
                        match expr[0] {
                            0xD2 => {
                                // ref.func instruction (0xD2 followed by funcidx)
                                if expr.len() > 1 {
                                    let (func_idx, _) = crate::instruction_parser::read_leb128_u32(expr, 1)?;
                                    #[cfg(feature = "tracing")]
                                    if i < 5 {
                                        trace!(elem_offset = offset_value + i as u32, func_idx = func_idx, "Element item = func (from ref.func)");
                                    }
                                    items.push(func_idx)?;
                                }
                            }
                            0xD0 => {
                                // ref.null instruction (0xD0 followed by heap type)
                                // Use u32::MAX as sentinel value to indicate null reference
                                #[cfg(feature = "tracing")]
                                if i < 5 {
                                    trace!(elem_offset = offset_value + i as u32, "Element item = ref.null");
                                }
                                items.push(u32::MAX)?;  // Sentinel for null reference
                            }
                            0x23 => {
                                // global.get instruction - defer evaluation
                                // Store the expression for later evaluation during element init
                                let (global_idx, _) = crate::instruction_parser::read_leb128_u32(expr, 1)?;
                                #[cfg(feature = "tracing")]
                                trace!(elem_offset = offset_value + i as u32, global_idx = global_idx, "Element item = global.get (deferred)");
                                let expr_insts = crate::instruction_parser::parse_instructions_with_provider(
                                    expr.as_slice(),
                                    shared_provider.clone()
                                )?;
                                deferred_item_exprs.push((i as u32, KilnExpr { instructions: expr_insts }));
                                // Push placeholder for deferred item
                                items.push(u32::MAX - 1)?;  // Sentinel for deferred evaluation
                            }
                            _ => {
                                // Non-trivial expression (e.g., i32.const + ref.i31, struct.new, etc.)
                                // Defer to item_exprs for full evaluation during initialization
                                #[cfg(feature = "tracing")]
                                trace!(elem_offset = offset_value + i as u32, opcode = format_args!("0x{:02X}", expr[0]), "Element item = expression (deferred)");
                                let expr_insts = crate::instruction_parser::parse_instructions_with_provider(
                                    expr.as_slice(),
                                    shared_provider.clone()
                                )?;
                                deferred_item_exprs.push((i as u32, KilnExpr { instructions: expr_insts }));
                                items.push(u32::MAX - 1)?;  // Sentinel for deferred evaluation
                            }
                        }
                    }
                },
            }

            // Create offset expression for the Element struct
            let offset_expr = if !elem_seg.offset_expr_bytes.is_empty() {
                let instructions = crate::instruction_parser::parse_instructions_with_provider(
                    elem_seg.offset_expr_bytes.as_slice(),
                    shared_provider.clone()
                )?;
                Some(KilnExpr { instructions })
            } else {
                None
            };

            #[cfg(feature = "tracing")]
            trace!(elem_idx = elem_idx, items_len = items.len(), mode = ?mode, "Element segment after conversion");

            let runtime_elem = Element {
                mode,
                table_idx,
                offset_expr,
                element_type: elem_seg.element_type,
                items,
                item_exprs: deferred_item_exprs,
            };

            runtime_module.elements.push(runtime_elem);
            #[cfg(feature = "tracing")]
            trace!(elem_idx = elem_idx, total_elements = runtime_module.elements.len(), "Element segment converted");
        }
        #[cfg(feature = "tracing")]
        debug!(total_elements = runtime_module.elements.len(), "Element segment conversion complete");

        // Pre-resolve element item values for all segments.
        // This evaluates constant expressions (ref.i31, struct.new, etc.) at module load time
        // so that table.init can use them directly for passive segments with GC-typed items.
        for elem_segment in &runtime_module.elements {
            let mut resolved = Vec::with_capacity(elem_segment.items.len());
            for i in 0..elem_segment.items.len() {
                let func_idx = elem_segment.items.get(i).unwrap_or(0);
                if func_idx == u32::MAX {
                    // ref.null sentinel
                    resolved.push(kiln_foundation::values::Value::FuncRef(None));
                } else if func_idx == u32::MAX - 1 {
                    // Deferred item — look up in item_exprs and evaluate
                    let mut found = false;
                    for (expr_idx, expr) in &elem_segment.item_exprs {
                        if *expr_idx == i as u32 {
                            // Evaluate the expression using available globals (non-import only)
                            use kiln_foundation::types::Instruction as KilnInstr;
                            use kiln_foundation::values::{Value as KilnValue, FuncRef as KilnFuncRef};
                            let mut eval_stack: Vec<KilnValue> = Vec::new();
                            for instr in &expr.instructions {
                                match instr {
                                    KilnInstr::I32Const(val) => {
                                        eval_stack.push(KilnValue::I32(*val));
                                    }
                                    KilnInstr::I64Const(val) => {
                                        eval_stack.push(KilnValue::I64(*val));
                                    }
                                    KilnInstr::RefI31 => {
                                        if let Some(KilnValue::I32(n)) = eval_stack.pop() {
                                            eval_stack.push(KilnValue::I31Ref(Some(n & 0x7FFFFFFF)));
                                        }
                                    }
                                    KilnInstr::RefFunc(func_idx) => {
                                        eval_stack.push(KilnValue::FuncRef(Some(KilnFuncRef::from_index(*func_idx))));
                                    }
                                    KilnInstr::RefNull(_) => {
                                        eval_stack.push(KilnValue::FuncRef(None));
                                    }
                                    KilnInstr::GlobalGet(_) => {
                                        // Needs instance globals -- push placeholder;
                                        // active segments re-resolve via item_exprs
                                        eval_stack.push(KilnValue::I32(0));
                                    }
                                    KilnInstr::End => break,
                                    _ => {}
                                }
                            }
                            if let Some(value) = eval_stack.pop() {
                                resolved.push(value);
                            } else {
                                resolved.push(KilnValue::FuncRef(None));
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        resolved.push(kiln_foundation::values::Value::FuncRef(None));
                    }
                } else {
                    // Normal function reference
                    resolved.push(kiln_foundation::values::Value::FuncRef(Some(
                        kiln_foundation::values::FuncRef::from_index(func_idx)
                    )));
                }
            }
            runtime_module.resolved_elem_items.push(resolved);
        }

        #[cfg(feature = "tracing")]
        {
            // Final check: verify element items are retained
            let elem_count = runtime_module.elements.len();
            if elem_count > 0 {
                let elem = &runtime_module.elements[0];
                trace!(elem_count = elem_count, first_elem_items_len = elem.items.len(), mode = ?elem.mode, "FINAL: element segments");
            } else {
                trace!("FINAL: elements.len()=0");
            }
        }

        #[cfg(feature = "tracing")]
        debug!("Bootstrap module conversion complete, returning runtime_module");
        Ok(Box::new(runtime_module))
    }


    /// Gets an export by name
    pub fn get_export(&self, name: &str) -> Option<Export> {
        // TODO: BoundedMap doesn't support iteration, so we'll use get with a
        // RuntimeString key
        let runtime_key: kiln_foundation::bounded::BoundedString<256> =
            kiln_foundation::bounded::BoundedString::from_str_truncate(name).ok()?;
        self.exports.get(&runtime_key).cloned()
    }

    /// Gets a function by index
    pub fn get_function(&self, idx: u32) -> Option<Function> {
        if idx as usize >= self.functions.len() {
            return None;
        }
        self.functions.get(idx as usize).cloned()
    }

    /// Helper method to push function
    pub fn push_function(&mut self, func: Function) -> Result<()> {
        self.functions.push(func);
        Ok(())
    }

    /// Gets a function type by index
    pub fn get_function_type(&self, idx: u32) -> Option<KilnFuncType> {
        if idx as usize >= self.types.len() {
            return None;
        }
        self.types.get(idx as usize).cloned()
    }

    /// Gets a global by index
    pub fn get_global(&self, idx: usize) -> Result<GlobalWrapper> {
        self.globals
            .get(idx)
            .cloned()
            .ok_or_else(|| Error::runtime_execution_error("Global index out of bounds"))
    }

    /// Gets a memory by index
    pub fn get_memory(&self, idx: usize) -> Result<&MemoryWrapper> {
        self.memories.get(idx).ok_or_else(|| {
            Error::new(
                ErrorCategory::Runtime,
                kiln_error::codes::MEMORY_NOT_FOUND,
                "Memory index out of bounds",
            )
        })
    }

    /// Gets a table by index
    pub fn get_table(&self, idx: usize) -> Result<TableWrapper> {
        self.tables
            .get(idx)
            .cloned()
            .ok_or_else(|| Error::runtime_execution_error("Table index out of bounds"))
    }

    /// Adds a function export
    pub fn add_function_export(&mut self, name: &str, index: u32) -> Result<()> {
        let export = Export::new(name, ExportKind::Function, index)?;
        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(name)?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Adds a table export
    pub fn add_table_export(&mut self, name: &str, index: u32) -> Result<()> {
        let export = Export::new(name, ExportKind::Table, index)?;
        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(name)?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Adds a memory export
    pub fn add_memory_export(&mut self, name: &str, index: u32) -> Result<()> {
        let export = Export::new(name, ExportKind::Memory, index)?;
        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(name)?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Adds a global export
    pub fn add_global_export(&mut self, name: &str, index: u32) -> Result<()> {
        let export = Export::new(name, ExportKind::Global, index)?;
        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(name)?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Adds an export to the module from a `kiln_format::module::Export`
    pub fn add_export(&mut self, format_export: kiln_format::module::Export) -> Result<()> {
        let runtime_export_kind = match format_export.kind {
            kiln_format::module::ExportKind::Function => ExportKind::Function,
            kiln_format::module::ExportKind::Table => ExportKind::Table,
            kiln_format::module::ExportKind::Memory => ExportKind::Memory,
            kiln_format::module::ExportKind::Global => ExportKind::Global,
            kiln_format::module::ExportKind::Tag => ExportKind::Tag,
        };
        // Convert BoundedString to String - use default empty string if conversion
        // fails
        let export_name_string = "export"; // Use a placeholder name
        let runtime_export =
            Export::new(export_name_string, runtime_export_kind, format_export.index)?;
        let name_key = kiln_foundation::bounded::BoundedString::from_str_truncate(
            runtime_export
                .name
                .as_str()
                .map_err(|_| Error::runtime_error("Invalid export name"))?
        )?;
        self.exports.insert(name_key, runtime_export)?;
        Ok(())
    }

    /// Set the name of the module
    pub fn set_name(&mut self, name: &str) -> Result<()> {
        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(
            name)?;
        self.name = Some(bounded_name);
        Ok(())
    }

    /// Set the start function index
    pub fn set_start(&mut self, start: u32) -> Result<()> {
        self.start = Some(start);
        Ok(())
    }

    /// Add a function type to the module
    pub fn add_type(&mut self, ty: KilnFuncType) -> Result<()> {
        self.types.push(ty);
        Ok(())
    }

    /// Add a function import to the module
    pub fn add_import_func(
        &mut self,
        module_name: &str,
        item_name: &str,
        type_idx: u32,
    ) -> Result<()> {
        let func_type = self
            .types
            .get(type_idx as usize)
            .cloned()
            .ok_or_else(|| Error::validation_type_mismatch("Type index out of bounds for import func"))?;

        let import_struct = crate::module::Import::new(
            module_name,
            item_name,
            ExternType::Func(func_type),
            RuntimeImportDesc::Function(0), // Function index would need to be determined properly
        )?;
        let bounded_module = kiln_foundation::bounded::BoundedString::from_str_truncate(
            module_name)?;
        let bounded_item = kiln_foundation::bounded::BoundedString::from_str_truncate(
            item_name)?;

        let mut inner_map = match self.imports.get(&bounded_module)? {
            Some(existing) => existing,
            None => ImportMap::new(create_runtime_provider()?)?,
        };

        let _ = inner_map.insert(bounded_item, import_struct)?;
        let _ = self.imports.insert(bounded_module, inner_map)?;
        Ok(())
    }

    /// Adds an imported table to the module
    pub fn add_import_table(
        &mut self,
        module_name: &str,
        item_name: &str,
        table_type: KilnTableType,
    ) -> Result<()> {
        let import_struct = crate::module::Import::new(
            module_name,
            item_name,
            ExternType::Table(table_type.clone()),
            RuntimeImportDesc::Table(table_type),
        )?;
        let bounded_module = kiln_foundation::bounded::BoundedString::from_str_truncate(
            module_name)?;
        let bounded_item = kiln_foundation::bounded::BoundedString::from_str_truncate(
            item_name)?;
        let mut inner_map = match self.imports.get(&bounded_module)? {
            Some(existing) => existing,
            None => ImportMap::new(create_runtime_provider()?)?,
        };
        let _ = inner_map.insert(bounded_item, import_struct)?;
        let _ = self.imports.insert(bounded_module, inner_map)?;
        Ok(())
    }

    /// Adds an imported memory to the module
    pub fn add_import_memory(
        &mut self,
        module_name: &str,
        item_name: &str,
        memory_type: KilnMemoryType,
    ) -> Result<()> {
        let import_struct = crate::module::Import::new(
            module_name,
            item_name,
            ExternType::Memory(memory_type),
            RuntimeImportDesc::Memory(memory_type),
        )?;
        let bounded_module = kiln_foundation::bounded::BoundedString::from_str_truncate(
            module_name)?;
        let bounded_item = kiln_foundation::bounded::BoundedString::from_str_truncate(
            item_name)?;
        let mut inner_map = match self.imports.get(&bounded_module)? {
            Some(existing) => existing,
            None => ImportMap::new(create_runtime_provider()?)?,
        };
        let _ = inner_map.insert(bounded_item, import_struct)?;
        let _ = self.imports.insert(bounded_module, inner_map)?;
        Ok(())
    }

    /// Adds an imported global to the module
    pub fn add_import_global(
        &mut self,
        module_name: &str,
        item_name: &str,
        format_global: kiln_format::module::Global,
    ) -> Result<()> {
        let component_global_type = kiln_foundation::types::GlobalType {
            value_type: format_global.global_type.value_type,
            mutable:    format_global.global_type.mutable,
        };

        let import = Import::new(
            module_name,
            item_name,
            ExternType::Global(component_global_type),
            RuntimeImportDesc::Global(component_global_type),
        )?;

        let module_key = kiln_foundation::bounded::BoundedString::from_str_truncate(
            module_name)?;
        let item_key = kiln_foundation::bounded::BoundedString::from_str_truncate(
            item_name)?;
        let provider = create_runtime_provider()?;
        let mut inner_map = BoundedMap::new(provider)?;
        inner_map.insert(item_key, import)?;
        self.imports.insert(module_key, inner_map)?;
        Ok(())
    }

    /// Add a function to the module
    pub fn add_function_type(&mut self, type_idx: u32) -> Result<()> {
        if type_idx as usize >= self.types.len() {
            return Err(Error::validation_type_mismatch(
                "Function type index out of bounds",
            ));
        }

        let provider = create_runtime_provider()?;
        let function = Function {
            type_idx,
            locals: kiln_foundation::bounded::BoundedVec::new(provider)?,
            body: KilnExpr::default(),
        };

        self.push_function(function)?;
        Ok(())
    }

    /// Add a table to the module
    pub fn add_table(&mut self, table_type: KilnTableType) -> Result<()> {
        let wrapper = TableWrapper::new(Table::new(table_type)?);
        self.tables.push(wrapper);
        Ok(())
    }

    /// Add a memory to the module
    pub fn add_memory(&mut self, memory_type: KilnMemoryType) -> Result<()> {
        self.push_memory(MemoryWrapper::new(Memory::new(to_core_memory_type(
            memory_type,
        ))?))?;
        Ok(())
    }

    /// Add a global to the module
    pub fn add_global(&mut self, global_type: KilnGlobalType, init: KilnValue) -> Result<()> {
        let global = Global::new(global_type.value_type, global_type.mutable, init)?;
        self.globals.push(GlobalWrapper::new(global));
        Ok(())
    }

    /// Add a function export to the module
    pub fn add_export_func(&mut self, name: &str, index: u32) -> Result<()> {
        if index as usize >= self.functions.len() {
            return Err(Error::validation_error(
                "Export function index out of bounds",
            ));
        }

        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(
            name)?;
        let export = Export::new(name, ExportKind::Function, index)?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Add a table export to the module
    pub fn add_export_table(&mut self, name: &str, index: u32) -> Result<()> {
        if index as usize >= self.tables.len() {
            return Err(Error::validation_error("Export table index out of bounds"));
        }

        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(
            name)?;
        let export = Export::new(bounded_name.as_str()?, ExportKind::Table, index)?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Add a memory export to the module
    pub fn add_export_memory(&mut self, name: &str, index: u32) -> Result<()> {
        if index as usize >= self.memories.len() {
            return Err(Error::validation_error("Export memory index out of bounds"));
        }

        let export = Export::new(name, ExportKind::Memory, index)?;

        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(
            name)?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Add a global export to the module
    pub fn add_export_global(&mut self, name: &str, index: u32) -> Result<()> {
        if index as usize >= self.globals.len() {
            return Err(Error::validation_error("Export global index out of bounds"));
        }

        let export = Export::new(name, ExportKind::Global, index)?;

        let bounded_name = kiln_foundation::bounded::BoundedString::from_str_truncate(
            name)?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Add an element segment to the module
    pub fn add_element(&mut self, element: kiln_format::module::Element) -> Result<()> {
        // Convert format element to runtime element
        let items = match &element.init {
            kiln_format::module::ElementInit::FuncIndices(func_indices) => {
                // For function indices, copy them
                let provider = create_runtime_provider()?;
                let mut bounded_items = kiln_foundation::bounded::BoundedVec::new(provider)?;
                for idx in func_indices.iter() {
                    bounded_items.push(*idx)?;
                }
                bounded_items
            },
            kiln_format::module::ElementInit::Expressions(_expressions) => {
                // For expressions, create empty items list for now (TODO: process expressions)
                let provider = create_runtime_provider()?;
                kiln_foundation::bounded::BoundedVec::new(provider)?
            },
        };

        // Extract table index from mode if available
        let table_idx = match &element.mode {
            kiln_format::pure_format_types::PureElementMode::Active { table_index, .. } => Some(*table_index),
            _ => None,
        };

        let runtime_element = crate::module::Element {
            mode: KilnElementMode::Active {
                table_index: 0,
                offset:      0,
            }, // Default mode, should be determined from element.mode
            table_idx,
            offset_expr: None, // Would need to convert from element.mode offset_expr
            element_type: element.element_type,
            items,
            item_exprs: Vec::new(),
        };

        self.elements.push(runtime_element);
        Ok(())
    }

    /// Set a function body
    pub fn set_function_body(
        &mut self,
        func_idx: u32,
        type_idx: u32,
        locals: Vec<KilnLocalEntry>,
        body: KilnExpr,
    ) -> Result<()> {
        if func_idx as usize > self.functions.len() {
            // Allow appending
            return Err(Error::runtime_function_not_found(
                "Function index out of bounds for set_function_body",
            ));
        }

        // Convert Vec<KilnLocalEntry> to BoundedVec
        let provider = create_runtime_provider()?;
        let mut bounded_locals =
            kiln_foundation::bounded::BoundedVec::<KilnLocalEntry, 256, RuntimeProvider>::new(
                provider,
            )?;
        for local in locals {
            bounded_locals.push(local)?;
        }

        let func_entry = Function {
            type_idx,
            locals: bounded_locals,
            body,
        };
        if func_idx as usize == self.functions.len() {
            self.push_function(func_entry)?;
        } else {
            if (func_idx as usize) < self.functions.len() {
                self.functions[func_idx as usize] = func_entry;
            } else {
                return Err(Error::runtime_component_limit_exceeded("Function index out of bounds"));
            }
        }
        Ok(())
    }

    /// Add a data segment to the module
    pub fn add_data(&mut self, data: kiln_format::pure_format_types::PureDataSegment) -> Result<()> {
        // Convert format data to runtime data
        let init_vec: Vec<u8> = data.data_bytes.clone();

        let runtime_data = crate::module::Data {
            mode:        KilnDataMode::Active {
                memory_index: 0,
                offset:       0,
            }, // Default mode
            memory_idx:  Some(0), // Default memory index - field is deprecated
            offset_expr: None,    // Would need to convert from data.offset
            init:        init_vec,
        };

        self.data.push(runtime_data);
        Ok(())
    }

    /// Add a custom section to the module
    pub fn add_custom_section(&mut self, name: &str, data: Vec<u8>) -> Result<()> {
        let name_key =
            kiln_foundation::bounded::BoundedString::from_str_truncate(name)?;
        let provider_data = create_runtime_provider()?;
        let mut bounded_data =
            kiln_foundation::bounded::BoundedVec::<u8, 4096, RuntimeProvider>::new(provider_data)?;
        for byte in data {
            bounded_data.push(byte)?;
        }
        self.custom_sections.insert(name_key, bounded_data)?;
        Ok(())
    }

    /// Set the binary representation of the module
    pub fn set_binary(&mut self, binary: Vec<u8>) -> Result<()> {
        self.binary = Some(binary);
        Ok(())
    }

    /// Validate the module
    pub fn validate(&self) -> Result<()> {
        // TODO: Implement comprehensive validation of the runtime module structure.
        // - Check type indices are valid.
        // - Check function indices in start/exports/elements are valid.
        // - Check table/memory/global indices.
        // - Validate instruction sequences in function bodies (optional, decoder should
        //   do most of this).
        Ok(())
    }

    /// Add an import runtime global to the module
    pub fn add_import_runtime_global(
        &mut self,
        module_name: &str,
        item_name: &str,
        global_type: KilnGlobalType,
    ) -> Result<()> {
        let component_global_type = kiln_foundation::types::GlobalType {
            value_type: global_type.value_type,
            mutable:    global_type.mutable,
        };
        let import_struct = crate::module::Import::new(
            module_name,
            item_name,
            ExternType::Global(component_global_type),
            RuntimeImportDesc::Global(component_global_type),
        )?;
        let bounded_module = kiln_foundation::bounded::BoundedString::from_str_truncate(
            module_name)?;
        let bounded_item = kiln_foundation::bounded::BoundedString::from_str_truncate(
            item_name)?;
        let mut inner_map = match self.imports.get(&bounded_module)? {
            Some(existing) => existing,
            None => ImportMap::new(create_runtime_provider()?)?,
        };
        let _ = inner_map.insert(bounded_item, import_struct)?;
        let _ = self.imports.insert(bounded_module, inner_map)?;
        Ok(())
    }

    /// Add a runtime export to the module
    pub fn add_runtime_export(&mut self, name: &str, export_desc: KilnExportDesc) -> Result<()> {
        let (kind, index) = match export_desc {
            KilnExportDesc::Func(idx) => (ExportKind::Function, idx),
            KilnExportDesc::Table(idx) => (ExportKind::Table, idx),
            KilnExportDesc::Mem(idx) => (ExportKind::Memory, idx),
            KilnExportDesc::Global(idx) => (ExportKind::Global, idx),
            KilnExportDesc::Tag(_) => {
                return Err(Error::not_supported_unsupported_operation(
                    "Tag exports not supported",
                ))
            },
        };
        let runtime_export = crate::module::Export::new(name, kind, index)?;
        let name_key = kiln_foundation::bounded::BoundedString::from_str_truncate(name)?;
        self.exports.insert(name_key, runtime_export)?;
        Ok(())
    }

    /// Add a runtime element to the module
    pub fn add_runtime_element(&mut self, element_segment: KilnElementSegment) -> Result<()> {
        // TODO: Resolve element_segment.items expressions if they are not direct
        // indices. This is a placeholder and assumes items can be derived or
        // handled during instantiation.
        // TODO: ElementItems type not available yet, using empty items for now
        let provider = create_runtime_provider()?;
        let items_resolved = kiln_foundation::bounded::BoundedVec::new(provider)?;

        // Convert element mode from PureElementMode to KilnElementMode
        let runtime_mode = match &element_segment.mode {
            kiln_format::pure_format_types::PureElementMode::Active { table_index, .. } => {
                KilnElementMode::Active {
                    table_index: *table_index,
                    offset:      0, // Simplified - would need to evaluate offset_expr_bytes
                }
            },
            kiln_format::pure_format_types::PureElementMode::Passive => KilnElementMode::Passive,
            kiln_format::pure_format_types::PureElementMode::Declared => KilnElementMode::Declarative,
        };

        self.elements.push(crate::module::Element {
            mode:         runtime_mode,
            table_idx:    None, // Simplified for now
            offset_expr:  None, // Element segment doesn't have direct offset_expr field
            element_type: element_segment.element_type,
            items:        items_resolved,
            item_exprs:   Vec::new(),
        });
        Ok(())
    }

    /// Add a runtime data segment to the module  
    pub fn add_runtime_data(&mut self, data_segment: KilnDataSegment) -> Result<()> {
        // KilnDataSegment is actually PureDataSegment
        // Convert data mode from PureDataMode to KilnDataMode
        let (runtime_mode, memory_idx) = match &data_segment.mode {
            kiln_format::pure_format_types::PureDataMode::Active { memory_index, .. } => {
                (
                    KilnDataMode::Active {
                        memory_index: *memory_index,
                        offset:       0, // Simplified - would need to evaluate offset_expr_bytes
                    },
                    Some(*memory_index),
                )
            },
            kiln_format::pure_format_types::PureDataMode::Passive => (KilnDataMode::Passive, None),
        };

        let runtime_init: Vec<u8> = data_segment.data_bytes.clone();

        self.data.push(crate::module::Data {
            mode: runtime_mode,
            memory_idx,
            offset_expr: None, // Simplified for now
            init: runtime_init,
        });
        Ok(())
    }

    /// Add a custom section to the module
    pub fn add_custom_section_runtime(
        &mut self,
        section: KilnCustomSection<RuntimeProvider>,
    ) -> Result<()> {
        let name_key = kiln_foundation::bounded::BoundedString::from_str_truncate(
            section.name.as_str()?
        )?;
        // Convert section.data to the expected type
        let provider_data = create_runtime_provider()?;
        let mut bounded_data =
            kiln_foundation::bounded::BoundedVec::<u8, 4096, RuntimeProvider>::new(provider_data)?;
        for i in 0..section.data.len() {
            bounded_data.push(section.data.get(i)?)?;
        }
        self.custom_sections.insert(name_key, bounded_data)?;
        Ok(())
    }

    /// Set the binary representation of the module (alternative method)
    pub fn set_binary_runtime(&mut self, binary: Vec<u8>) -> Result<()> {
        self.binary = Some(binary);
        Ok(())
    }

    /// Load a module from WebAssembly binary
    ///
    /// This method uses streaming decoding to minimize memory usage.
    /// The binary is processed section by section without loading
    /// the entire module into intermediate data structures.
    pub fn load_from_binary(&mut self, binary: &[u8]) -> Result<Self> {
        // Use kiln-decoder's unified loader for efficient parsing
        use kiln_decoder::{
            load_wasm_unified,
            WasmFormat,
        };

        // Load using unified API to get both module info and cached data
        let wasm_info = load_wasm_unified(binary)?;

        // Ensure this is a core module
        if !wasm_info.is_core_module() {
            return Err(Error::validation_type_mismatch(
                "Binary is not a WebAssembly core module",
            ));
        }

        #[cfg(feature = "tracing")]
        trace!("About to call require_module_info");
        let module_info = wasm_info.require_module_info()?;
        #[cfg(feature = "tracing")]
        trace!("Got module_info successfully");

        // Create runtime module from unified API data
        #[cfg(feature = "tracing")]
        trace!("About to call from_module_info");
        let runtime_module = Self::from_module_info(module_info, binary)?;
        #[cfg(feature = "tracing")]
        trace!("from_module_info completed successfully");

        // Store the binary for later use
        let bounded_binary = binary.to_vec();

        Ok(Self {
            binary: Some(bounded_binary),
            validated: true,
            ..runtime_module
        })
    }

    /// Create runtime Module from unified API ModuleInfo
    fn from_module_info(module_info: &kiln_decoder::ModuleInfo, binary: &[u8]) -> Result<Self> {
        // Create module directly using create_runtime_provider
        let provider = crate::bounded_runtime_infra::create_runtime_provider()?;
        let mut runtime_module = Self {
            types: Vec::new(),
            imports: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            import_order: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: Vec::new(),
            tags: Vec::new(),
            elements: Vec::new(),
            resolved_elem_items: Vec::new(),
            data: Vec::new(),
            start: None,
            custom_sections: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            exports: kiln_foundation::direct_map::DirectMap::new(),
            name: None,
            binary: None,
            validated: false,
            num_global_imports: 0,
            global_import_types: Vec::new(),
            deferred_global_inits: Vec::new(),
            import_types: Vec::new(),
            num_import_functions: 0,
            gc_types: Vec::new(),
            type_supertypes: Vec::new(),
            table_init_exprs: Vec::new(),
        };

        // Set start function if present
        runtime_module.start = module_info.start_function;

        // Process imports
        for import in &module_info.imports {
            let extern_type = match &import.import_type {
                kiln_decoder::ImportType::Function(type_idx) => {
                    // For now, create a simple function type
                    // In a full implementation, we'd look up the actual type
                    let func_type = KilnFuncType::new(
                        core::iter::empty::<KilnValueType>(), // empty params
                        core::iter::empty::<KilnValueType>(), // empty results
                    )?;
                    ExternType::Func(func_type)
                },
                kiln_decoder::ImportType::Table => {
                    // Create default table type
                    let table_type = KilnTableType {
                        element_type: KilnRefType::Funcref,
                        limits:       KilnLimits { min: 0, max: None },
                        table64:      false,
                    };
                    ExternType::Table(table_type)
                },
                kiln_decoder::ImportType::Memory => {
                    // CRITICAL: Memory imports in Component Model should NOT have hardcoded limits
                    // The actual memory specifications come from the provider module (Module 0 with 2 pages)
                    // For shared-everything dynamic linking, all modules share the same linear memory
                    // Temporarily use min:0 to indicate this needs to be resolved via linking
                    let memory_type = KilnMemoryType {
                        limits: KilnLimits { min: 0, max: None },  // Will be resolved via linking
                        shared: true,  // Component Model uses shared memory
                        memory64: false,
                        page_size: None,
                    };
                    ExternType::Memory(memory_type)
                },
                kiln_decoder::ImportType::Global => {
                    // Create default global type
                    let global_type = kiln_foundation::types::GlobalType {
                        value_type: KilnValueType::I32,
                        mutable:    false,
                    };
                    ExternType::Global(global_type)
                },
            };

            // Create the import
            let import_struct = crate::module::Import::new(
                import.module.as_str(),
                import.name.as_str(),
                extern_type.clone(),
                match &extern_type {
                    ExternType::Func(_func_type) => RuntimeImportDesc::Function(0), /* TODO: proper type index lookup */
                    ExternType::Table(table_type) => RuntimeImportDesc::Table(table_type.clone()),
                    ExternType::Memory(memory_type) => {
                        RuntimeImportDesc::Memory(*memory_type)
                    },
                    ExternType::Global(global_type) => {
                        RuntimeImportDesc::Global(*global_type)
                    },
                    ExternType::Tag(_) => RuntimeImportDesc::Function(0), /* Handle tag as function placeholder */
                    ExternType::Component(_) => RuntimeImportDesc::Function(0), /* Component imports not supported yet */
                    ExternType::Instance(_) => RuntimeImportDesc::Function(0), /* Instance imports not supported yet */
                    ExternType::CoreModule(_) => RuntimeImportDesc::Function(0), /* Core module imports not supported yet */
                    ExternType::TypeDef(_) => RuntimeImportDesc::Function(0), /* Type definition imports not supported yet */
                    ExternType::Resource(_) => RuntimeImportDesc::Function(0), /* Resource imports not supported yet */
                },
            )?;

            // Add to imports map
            #[cfg(feature = "tracing")]
            trace!(module = %import.module, "Creating module_key");
            let module_key = kiln_foundation::bounded::BoundedString::from_str_truncate(
                &import.module)
                .map_err(|e| {
                    #[cfg(feature = "tracing")]
                    warn!(error = ?e, "Failed to create module_key");
                    Error::foundation_bounded_capacity_exceeded("Failed to convert module name")
                })?;
            #[cfg(feature = "tracing")]
            trace!(name = %import.name, "Creating item_key");
            let item_key = kiln_foundation::bounded::BoundedString::from_str_truncate(
                &import.name)
                .map_err(|e| {
                    #[cfg(feature = "tracing")]
                    warn!(error = ?e, "Failed to create item_key");
                    Error::foundation_bounded_capacity_exceeded("Failed to convert import name")
                })?;

            // Get or create inner map
            #[cfg(feature = "tracing")]
            trace!("Getting or creating inner map for module_key");
            let mut inner_map = match runtime_module.imports.get(&module_key)? {
                Some(existing) => {
                    #[cfg(feature = "tracing")]
                    trace!("Found existing inner map");
                    existing
                },
                None => {
                    #[cfg(feature = "tracing")]
                    trace!("Creating new inner map");
                    ImportMap::new(create_runtime_provider()?)?
                },
            };

            // Insert the import
            #[cfg(feature = "tracing")]
            trace!(module = ?import_struct.module.as_str(), name = ?import_struct.name.as_str(), "Inserting import into inner map");
            inner_map.insert(item_key, import_struct).map_err(|e| {
                #[cfg(feature = "tracing")]
                warn!(error = ?e, "Insert failed");
                e
            })?;
            #[cfg(feature = "tracing")]
            trace!("Inserting inner map into imports");
            runtime_module.imports.insert(module_key, inner_map)?;

            // Track import order for index-based lookup
            runtime_module.import_order.push((import.module.clone(), import.name.clone()));

            #[cfg(feature = "tracing")]
            trace!("Import processed successfully");
        }

        // Count function imports and set num_import_functions
        runtime_module.num_import_functions = module_info.imports.iter()
            .filter(|imp| matches!(imp.import_type, kiln_decoder::ImportType::Function(_)))
            .count();

        // Process exports
        for export in &module_info.exports {
            let export_kind = match export.export_type {
                kiln_decoder::ExportType::Function => ExportKind::Function,
                kiln_decoder::ExportType::Table => ExportKind::Table,
                kiln_decoder::ExportType::Memory => ExportKind::Memory,
                kiln_decoder::ExportType::Global => ExportKind::Global,
            };

            let runtime_export = Export::new(&export.name, export_kind, export.index)?;
            let name_key = kiln_foundation::bounded::BoundedString::from_str_truncate(
                &export.name)?;
            runtime_module.exports.insert(name_key, runtime_export)?;
        }

        // Set memory info if present
        if let Some((min_pages, max_pages)) = module_info.memory_pages {
            let memory_type = KilnMemoryType {
                limits: KilnLimits {
                    min: min_pages,
                    max: max_pages,
                },
                shared: false,
                memory64: false,
                page_size: None,
            };
            runtime_module
                .push_memory(MemoryWrapper::new(Memory::new(to_core_memory_type(
                    memory_type,
                ))?))?;
        }

        // For now, we'll use the fallback decoder for full section parsing if needed
        // This ensures compatibility while leveraging the unified API for basic info
        if !module_info.function_types.is_empty() {
            // Fall back to full parsing for complex cases
            use kiln_decoder::decoder;
            let decoded_module = Box::new(decoder::decode_module(binary)?);

            // decoded_module is kiln_format::Module, so we need the format-compatible method
            let full_runtime_module = *Module::from_kiln_module(&*decoded_module)?;

            return Ok(full_runtime_module);
        }

        Ok(runtime_module)
    }

    /// Find a function export by name
    pub fn find_function_by_name(&self, name: &str) -> Option<u32> {
        #[cfg(feature = "tracing")]
        trace!(name = name, exports_len = self.exports.len(), "[FIND_FUNC] Looking up export");

        let bounded_name =
            kiln_foundation::bounded::BoundedString::from_str_truncate(name).ok()?;

        if let Some(export) = self.exports.get(&bounded_name) {
            #[cfg(feature = "tracing")]
            trace!(kind = ?export.kind, index = export.index, "[FIND_FUNC] Found export");
            if export.kind == ExportKind::Function {
                return Some(export.index);
            }
        }

        #[cfg(feature = "tracing")]
        trace!(name = name, "[FIND_FUNC] Export not found or not a function");
        None
    }

    /// Get function signature by function index
    pub fn get_function_signature(&self, func_idx: u32) -> Option<KilnFuncType> {
        let function = self.get_function(func_idx)?;
        self.get_function_type(function.type_idx)
    }

    /// Validate that a function exists and can be called
    pub fn validate_function_call(&self, name: &str) -> Result<u32> {
        match self.find_function_by_name(name) {
            Some(func_idx) => {
                // Verify function exists
                if self.get_function(func_idx).is_some() {
                    Ok(func_idx)
                } else {
                    Err(Error::runtime_function_not_found(
                        "Function index is invalid",
                    ))
                }
            },
            None => Err(Error::runtime_function_not_found(
                "Function not found in exports",
            )),
        }
    }

    /// Get the import identity for a tag (module_name, field_name)
    /// Returns Some((module, name)) if the tag is imported, None if it's locally defined
    pub fn get_tag_import_identity(&self, tag_idx: u32) -> Option<(String, String)> {
        let num_tag_imports = self.count_tag_imports();
        if (tag_idx as usize) < num_tag_imports {
            // This is an imported tag - find the corresponding import entry
            let mut tag_import_idx = 0;
            let mut import_order_idx = 0;
            for desc in &self.import_types {
                if let RuntimeImportDesc::Tag(_) = desc {
                    if tag_import_idx == tag_idx as usize {
                        // Found the tag import - get the module/name from import_order
                        if let Some((module, name)) = self.import_order.get(import_order_idx) {
                            return Some((module.clone(), name.clone()));
                        }
                    }
                    tag_import_idx += 1;
                }
                import_order_idx += 1;
            }
            None
        } else {
            // This is a locally defined tag, not imported
            None
        }
    }

    /// Get the export name for a locally defined tag
    /// Returns Some(export_name) if the tag is exported, None otherwise
    pub fn get_tag_export_name(&self, tag_idx: u32) -> Option<String> {
        // Look through exports to find if this tag is exported
        for export in self.exports.values() {
            if export.kind == ExportKind::Tag && export.index == tag_idx {
                // Found the export - return the name
                if let Ok(name_str) = export.name.as_str() {
                    return Some(name_str.to_string());
                }
            }
        }
        None
    }
}

/// Additional exports that are not part of the standard WebAssembly exports
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherExport {
    /// Export name
    pub name:  kiln_foundation::bounded::BoundedString<128>,
    /// Export kind
    pub kind:  ExportKind,
    /// Export index
    pub index: u32,
}

/// Represents an imported item in a WebAssembly module
#[derive(Debug, Clone)]
pub enum ImportedItem {
    /// An imported function
    Function {
        /// The module name
        module: kiln_foundation::bounded::BoundedString<128>,
        /// The function name
        name:   kiln_foundation::bounded::BoundedString<128>,
        /// The function type
        ty:     KilnFuncType,
    },
    /// An imported table
    Table {
        /// The module name
        module: kiln_foundation::bounded::BoundedString<128>,
        /// The table name
        name:   kiln_foundation::bounded::BoundedString<128>,
        /// The table type
        ty:     KilnTableType,
    },
    /// An imported memory
    Memory {
        /// The module name
        module: kiln_foundation::bounded::BoundedString<128>,
        /// The memory name
        name:   kiln_foundation::bounded::BoundedString<128>,
        /// The memory type
        ty:     KilnMemoryType,
    },
    /// An imported global
    Global {
        /// The module name
        module: kiln_foundation::bounded::BoundedString<128>,
        /// The global name
        name:   kiln_foundation::bounded::BoundedString<128>,
        /// The global type
        ty:     KilnGlobalType,
    },
}

// Trait implementations for Module
impl kiln_foundation::traits::Checksummable for Module {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        // Use module name (if available) and validation status for checksum
        if let Some(ref name) = self.name {
            if let Ok(name_str) = name.as_str() {
                checksum.update_slice(name_str.as_bytes());
            }
        } else {
            // Use a default identifier if no name is available
            checksum.update_slice(b"unnamed_module");
        }
        checksum.update_slice(&[if self.validated { 1 } else { 0 }]);
        checksum.update_slice(&(self.types.len() as u32).to_le_bytes());
        checksum.update_slice(&(self.functions.len() as u32).to_le_bytes());
    }
}

impl kiln_foundation::traits::ToBytes for Module {
    fn serialized_size(&self) -> usize {
        // Simple size calculation for module metadata
        let name_size = self.name.as_ref().map_or(0, |n| n.len());
        8 + name_size + 1 + 4 + 4 // magic(4) + name_len(4) + name +
                                  // validated(1) + types_len(4) +
                                  // functions_len(4)
    }

    fn to_bytes_with_provider<'a, PStream: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> Result<()> {
        // Write a magic number to identify this as a module
        writer.write_all(&0x6D6F6475u32.to_le_bytes())?; // "modu" in little endian

        // Write module name length and name
        if let Some(ref name) = self.name {
            if let Ok(name_str) = name.as_str() {
                writer.write_all(&(name_str.len() as u32).to_le_bytes())?;
                writer.write_all(name_str.as_bytes())?;
            } else {
                writer.write_all(&0u32.to_le_bytes())?; // Error getting name
            }
        } else {
            writer.write_all(&0u32.to_le_bytes())?; // No name
        }

        // Write validation status
        writer.write_all(&[if self.validated { 1 } else { 0 }])?;

        // Write type and function counts
        writer.write_all(&(self.types.len() as u32).to_le_bytes())?;
        writer.write_all(&(self.functions.len() as u32).to_le_bytes())?;

        Ok(())
    }
}

impl kiln_foundation::traits::FromBytes for Module {
    fn from_bytes_with_provider<'a, PStream: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> Result<Self> {
        // Read and verify magic number
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if u32::from_le_bytes(magic) != 0x6D6F6475 {
            return Err(kiln_error::Error::runtime_error(
                "Invalid module magic number",
            ));
        }

        // Read module name
        let mut name_len_bytes = [0u8; 4];
        reader.read_exact(&mut name_len_bytes)?;
        let name_len = u32::from_le_bytes(name_len_bytes);

        let name = if name_len > 0 && name_len <= 128 {
            // Use a fixed-size buffer for reading the name
            let mut name_bytes = [0u8; 128];
            reader.read_exact(&mut name_bytes[..name_len as usize])?;
            let name_str = core::str::from_utf8(&name_bytes[..name_len as usize])
                .map_err(|_| kiln_error::Error::runtime_error("Invalid module name UTF-8"))?;
            Some(kiln_foundation::bounded::BoundedString::try_from_str(
                name_str
            )?)
        } else {
            None
        };

        // Read validation status
        let mut validated_byte = [0u8; 1];
        reader.read_exact(&mut validated_byte)?;
        let validated = validated_byte[0] != 0;

        // Read type and function counts (for validation)
        let mut counts = [0u8; 8];
        reader.read_exact(&mut counts)?;

        // Create a new empty module with the restored name and validation status
        let provider = crate::bounded_runtime_infra::create_runtime_provider()?;
        let module = Module {
            types: Vec::new(),
            imports: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            import_order: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: Vec::new(),
            tags: Vec::new(),
            elements: Vec::new(),
            resolved_elem_items: Vec::new(),
            data: Vec::new(),
            start: None,
            custom_sections: kiln_foundation::bounded_collections::BoundedMap::new(provider.clone())?,
            exports: kiln_foundation::direct_map::DirectMap::new(),
            name,
            binary: None,
            validated,
            num_global_imports: 0,
            global_import_types: Vec::new(),
            deferred_global_inits: Vec::new(),
            import_types: Vec::new(),
            num_import_functions: 0,
            gc_types: Vec::new(),
            type_supertypes: Vec::new(),
            table_init_exprs: Vec::new(),
        };

        Ok(module)
    }
}

// HashMap is already imported above, no need to re-import

use kiln_error::{
    Error,
    ErrorCategory,
    Result,
};
use kiln_foundation::component::ExternType; // For error handling

// Newtype wrappers to solve orphan rules issue
// These allow us to implement external traits on types containing Arc<T>

/// Wrapper for Arc<Table> to enable trait implementations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableWrapper(pub Arc<Table>);

impl Default for TableWrapper {
    fn default() -> Self {
        use kiln_foundation::types::{
            Limits,
            RefType,
            TableType,
        };
        let table_type = TableType {
            element_type: RefType::Funcref,
            limits:       Limits {
                min: 0,
                max: Some(1),
            },
            table64: false,
        };
        Self::new(Table::new(table_type).expect("Failed to create Table for TableWrapper::default()"))
    }
}

impl TableWrapper {
    /// Create a new table wrapper
    pub fn new(table: Table) -> Self {
        Self(Arc::new(table))
    }

    /// Get a reference to the inner table
    #[must_use]
    pub fn inner(&self) -> &Arc<Table> {
        &self.0
    }

    /// Unwrap to get the Arc<Table>
    #[must_use]
    pub fn into_inner(self) -> Arc<Table> {
        self.0
    }

    /// Get table size
    #[must_use]
    pub fn size(&self) -> u32 {
        self.0.size()
    }

    /// Get table element
    pub fn get(&self, idx: u32) -> Result<Option<KilnValue>> {
        self.0.get(idx)
    }

    /// Set table element using interior mutability
    pub fn set(&self, idx: u32, value: Option<KilnValue>) -> Result<()> {
        // Use set_shared which uses the internal Mutex for interior mutability
        self.0.set_shared(idx, value)
    }

    /// Grow table using interior mutability
    pub fn grow(&self, delta: u32, init_value: KilnValue) -> Result<u32> {
        self.0.grow_shared(delta, init_value)
    }

    /// Initialize table using interior mutability
    pub fn init(&self, offset: u32, init_data: &[Option<KilnValue>]) -> Result<()> {
        self.0.init_shared(offset, init_data)
    }

    /// Fill a range of table elements with a value
    pub fn fill(&self, offset: u32, len: u32, value: Option<KilnValue>) -> Result<()> {
        self.0.fill_elements_shared(offset as usize, value, len as usize)
    }

    /// Copy elements from one region to another
    pub fn copy(&self, dst: u32, src: u32, len: u32) -> Result<()> {
        self.0.copy_elements_shared(dst as usize, src as usize, len as usize)
    }

    /// Get the table element type
    pub fn element_type(&self) -> kiln_foundation::types::RefType {
        self.0.ty.element_type
    }

    /// Check if this table uses 64-bit indices (table64 proposal)
    #[must_use]
    pub fn is_table64(&self) -> bool {
        self.0.ty.table64
    }
}

/// Wrapper for Arc<Memory> to enable trait implementations  
/// Memory guard for atomic operations
#[derive(Debug)]
pub struct MemoryGuard {
    memory: Arc<Memory>,
}

impl MemoryGuard {
    /// Read from memory
    pub fn read(&self, offset: usize, buffer: &mut [u8]) -> Result<()> {
        self.memory.read(offset as u32, buffer)
    }

    /// Write to memory (atomic operations may need this)
    pub fn write(&self, offset: usize, buffer: &[u8]) -> Result<()> {
        // TODO: Implement safe atomic memory write operations for Arc<Memory>
        // For now, return an error as Arc<Memory> doesn't allow mutable access
        Err(Error::runtime_execution_error(
            "Atomic memory write operations not yet implemented for Arc<Memory>",
        ))
    }
}

/// Wrapper for shared memory instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryWrapper(pub Arc<Memory>);

impl Default for MemoryWrapper {
    fn default() -> Self {
        // FAIL LOUD AND EARLY: This violates the NO FALLBACK LOGIC rule from CLAUDE.md
        // Memory must always be explicitly created or shared - there should never be
        // a fallback default memory.
        //
        // This Default is only provided because BoundedVec requires it, but it should
        // NEVER be used in practice. If this panic fires, it means:
        // - Wrapper modules aren't properly inheriting memory from Module 0
        // - Memory specifications weren't being properly linked during instantiation
        // - Instances are being created without required memory
        panic!(
            "CRITICAL: MemoryWrapper::default() called - this indicates a memory linking bug. \
             Memories must be explicitly created or shared, never defaulted. \
             Check that all modules properly inherit or share memory from Module 0."
        );
    }
}

impl AsRef<Arc<Memory>> for MemoryWrapper {
    fn as_ref(&self) -> &Arc<Memory> {
        &self.0
    }
}

impl MemoryWrapper {
    /// Create a new memory wrapper
    pub fn new(memory: Box<Memory>) -> Self {
        Self(Arc::from(memory))
    }

    /// Get a reference to the inner memory
    #[must_use]
    pub fn inner(&self) -> &Arc<Memory> {
        &self.0
    }

    /// Unwrap to get the Arc<Memory>
    #[must_use]
    pub fn into_inner(self) -> Arc<Memory> {
        self.0
    }

    /// Get memory size in bytes
    #[must_use]
    pub fn size_in_bytes(&self) -> usize {
        self.0.size_in_bytes()
    }

    /// Get memory size in pages
    #[must_use]
    pub fn size(&self) -> u32 {
        self.0.size()
    }

    /// Get memory size in pages (alias for compatibility)
    #[must_use]
    pub fn size_pages(&self) -> u32 {
        self.0.size()
    }

    /// Get memory size in bytes (alias for compatibility)
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        self.0.size_in_bytes()
    }

    /// Read from memory
    pub fn read(&self, offset: u32, buffer: &mut [u8]) -> Result<()> {
        self.0.read(offset, buffer)
    }

    /// Write to memory (requires mutable access to Arc<Memory>)
    pub fn write(&self, offset: u32, buffer: &[u8]) -> Result<()> {
        // Note: This requires unsafe because we can't get mutable access to Arc<Memory>
        // For now, we'll return an error
        Err(Error::new(
            ErrorCategory::Runtime,
            kiln_error::codes::MEMORY_ACCESS_DENIED,
            "Cannot write to memory through Arc<Memory>",
        ))
    }

    /// Grow memory (requires mutable access)
    pub fn grow(&self, pages: u32) -> Result<u32> {
        // Note: This requires unsafe because we can't get mutable access to Arc<Memory>
        // For now, we'll return an error
        Err(Error::runtime_execution_error(
            "Runtime execution error: Cannot grow memory through Arc<Memory>",
        ))
    }

    /// Write i32 to memory
    pub fn write_i32(&self, offset: u32, value: i32) -> Result<()> {
        use crate::memory_helpers::ArcMemoryExt;
        self.0.write_i32(offset, value)
    }

    /// Write i64 to memory
    pub fn write_i64(&self, offset: u32, value: i64) -> Result<()> {
        use crate::memory_helpers::ArcMemoryExt;
        self.0.write_i64(offset, value)
    }

    /// Write f32 to memory
    pub fn write_f32(&self, offset: u32, value: f32) -> Result<()> {
        use crate::memory_helpers::ArcMemoryExt;
        self.0.write_f32(offset, value)
    }

    /// Write f64 to memory
    pub fn write_f64(&self, offset: u32, value: f64) -> Result<()> {
        use crate::memory_helpers::ArcMemoryExt;
        self.0.write_f64(offset, value)
    }

    /// Fill memory (requires mutable access)
    pub fn fill(&self, offset: u32, len: u32, value: u8) -> Result<()> {
        // Note: This requires unsafe because we can't get mutable access to Arc<Memory>
        // For now, we'll return an error
        Err(Error::new(
            ErrorCategory::Runtime,
            kiln_error::codes::MEMORY_ACCESS_DENIED,
            "Cannot fill memory through Arc<Memory>",
        ))
    }

    /// Get a memory guard for atomic operations
    pub fn lock(&self) -> MemoryGuard {
        MemoryGuard {
            memory: self.0.clone(),
        }
    }
}

/// Wrapper for Arc<RwLock<Global>> to enable trait implementations and mutable access
#[derive(Debug, Clone)]
pub struct GlobalWrapper(pub Arc<RwLock<Global>>);

// Manual PartialEq implementation since RwLock doesn't implement PartialEq
impl PartialEq for GlobalWrapper {
    fn eq(&self, other: &Self) -> bool {
        // Compare by Arc pointer equality (same underlying lock)
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for GlobalWrapper {}

impl Default for GlobalWrapper {
    fn default() -> Self {
        use kiln_foundation::{
            types::ValueType,
            values::Value,
        };
        Self::new(Global::new(ValueType::I32, false, Value::I32(0))
            .expect("Failed to create Global for GlobalWrapper::default()"))
    }
}

impl GlobalWrapper {
    /// Create a new global wrapper
    pub fn new(global: Global) -> Self {
        Self(Arc::new(RwLock::new(global)))
    }

    /// Get a reference to the inner global (returns the Arc<RwLock<Global>>)
    #[must_use]
    pub fn inner(&self) -> &Arc<RwLock<Global>> {
        &self.0
    }

    /// Get the global value
    pub fn get(&self) -> Result<KilnValue> {
        let guard = self.0.read().map_err(|_| {
            crate::Error::runtime_execution_error("Failed to acquire read lock on global")
        })?;
        Ok(guard.get().clone())
    }

    /// Set the global value
    pub fn set(&self, value: KilnValue) -> Result<()> {
        let mut guard = self.0.write().map_err(|_| {
            crate::Error::runtime_execution_error("Failed to acquire write lock on global")
        })?;

        #[cfg(feature = "tracing")]
        {
            use kiln_foundation::tracing::debug;
            let global_type = guard.global_type_descriptor();
            debug!(
                "GlobalWrapper::set - global is mutable: {}, value_type: {:?}, new_value: {:?}",
                global_type.mutable, global_type.value_type, value
            );
        }

        guard.set(&value)
    }

    /// Unwrap to get the Arc<RwLock<Global>>
    #[must_use]
    pub fn into_inner(self) -> Arc<RwLock<Global>> {
        self.0
    }

    /// Get global value (returns a clone of the value)
    pub fn get_value(&self) -> KilnValue {
        let guard = self.0.read().unwrap_or_else(|e| e.into_inner());
        guard.get().clone()
    }

    /// Set global value (requires mutable access)
    pub fn set_value(&self, new_value: &KilnValue) -> Result<()> {
        let mut guard = self.0.write().map_err(|_| {
            crate::Error::runtime_execution_error("Failed to acquire write lock on global")
        })?;
        guard.set(new_value)
    }

    /// Get global value type
    #[must_use]
    pub fn value_type(&self) -> KilnValueType {
        let guard = self.0.read().unwrap_or_else(|e| e.into_inner());
        guard.global_type_descriptor().value_type
    }

    /// Check if global is mutable
    #[must_use]
    pub fn is_mutable(&self) -> bool {
        let guard = self.0.read().unwrap_or_else(|e| e.into_inner());
        guard.global_type_descriptor().mutable
    }
}

// Implement foundation traits for wrapper types
use kiln_foundation::{
    traits::{
        ReadStream,
        WriteStream,
    },
    verification::Checksum,
};

// TableWrapper trait implementations
impl Checksummable for TableWrapper {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Use table size and element type for checksum
        checksum.update_slice(&self.0.size().to_le_bytes());
        let element_type_byte: u8 = match self.0.ty.element_type {
            KilnRefType::Funcref => 0,
            KilnRefType::Externref => 1,
            KilnRefType::Gc(_) => 2,
        };
        checksum.update_slice(&element_type_byte.to_le_bytes());
    }
}

impl ToBytes for TableWrapper {
    fn serialized_size(&self) -> usize {
        12 // table type (4) + size (4) + limits (4)
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&self.0.size().to_le_bytes())?;
        let element_type_byte: u8 = match self.0.ty.element_type {
            KilnRefType::Funcref => 0,
            KilnRefType::Externref => 1,
            KilnRefType::Gc(_) => 2,
        };
        writer.write_all(&element_type_byte.to_le_bytes())?;
        writer.write_all(&self.0.ty.limits.min.to_le_bytes())?;
        Ok(())
    }
}

impl FromBytes for TableWrapper {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 12];
        reader.read_exact(&mut bytes)?;

        // Create a default table (simplified implementation)
        use kiln_foundation::types::{
            Limits,
            RefType,
            TableType,
        };
        let table_type = TableType {
            element_type: RefType::Funcref,
            limits:       Limits {
                min: 0,
                max: Some(1),
            },
            table64: false,
        };

        let table = Table::new(table_type).map_err(|_| {
            kiln_error::Error::runtime_execution_error(
                "Runtime execution error: Failed to create table from bytes",
            )
        })?;

        Ok(TableWrapper::new(table))
    }
}

// MemoryWrapper trait implementations
// EMERGENCY FIX: Implement StaticSerializedSize to avoid recursion
impl kiln_foundation::traits::StaticSerializedSize for MemoryWrapper {
    const SERIALIZED_SIZE: usize = 12; // size (4) + limits min (4) + limits max (4)
}

// Note: BoundedVec specialization is handled through StaticSerializedSize trait

impl Checksummable for MemoryWrapper {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Use memory size for checksum
        checksum.update_slice(&self.0.size().to_le_bytes());
        checksum.update_slice(&self.0.size_in_bytes().to_le_bytes());
    }
}

impl ToBytes for MemoryWrapper {
    fn serialized_size(&self) -> usize {
        // Use static size to avoid recursion in Default::default().serialized_size()
        // calls
        <Self as kiln_foundation::traits::StaticSerializedSize>::SERIALIZED_SIZE
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&self.0.size().to_le_bytes())?;
        writer.write_all(&self.0.ty.limits.min.to_le_bytes())?;
        let max = self.0.ty.limits.max.unwrap_or(u32::MAX);
        writer.write_all(&max.to_le_bytes())?;
        Ok(())
    }
}

impl FromBytes for MemoryWrapper {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 12];
        reader.read_exact(&mut bytes)?;

        // FAIL LOUD AND EARLY: This violates the NO FALLBACK LOGIC rule
        // Memories must be shared from Module 0, not created during deserialization
        panic!(
            "CRITICAL: MemoryWrapper::from_bytes called - this indicates a memory linking bug. \
             Wrapper modules should inherit memory from Module 0 through shared-everything linking, \
             not create new memories during deserialization. The hardcoded 1-page memory (min: 1, max: Some(1)) \
             was masking a bug where wrapper modules weren't properly sharing Module 0's memory."
        );
    }
}

// Helper function to convert ValueType to u8
fn value_type_to_u8(vt: KilnValueType) -> u8 {
    match vt {
        KilnValueType::I32 => 0,
        KilnValueType::I64 => 1,
        KilnValueType::F32 => 2,
        KilnValueType::F64 => 3,
        KilnValueType::FuncRef => 4,
        KilnValueType::ExternRef => 5,
        KilnValueType::V128 => 6,
        KilnValueType::I16x8 => 7,
        KilnValueType::StructRef(_) => 8,
        KilnValueType::ArrayRef(_) => 9,
        KilnValueType::ExnRef => 10,
        KilnValueType::I31Ref => 11,
        KilnValueType::AnyRef => 12,
        KilnValueType::EqRef => 13,
        // TypedFuncRef is a concrete function reference type (ref null? $type)
        // Serialize as FuncRef (4) since the value representation is identical
        KilnValueType::TypedFuncRef(_, _) => 4,
        KilnValueType::NullFuncRef => 4,
        KilnValueType::NoneRef => 14,       // Bottom of any hierarchy
        KilnValueType::NoExternRef => 15,   // Bottom of extern hierarchy
        KilnValueType::NoExnRef => 16,      // Bottom of exn hierarchy
    }
}

// GlobalWrapper trait implementations
impl Checksummable for GlobalWrapper {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Use global value type for checksum
        let guard = self.0.read().unwrap_or_else(|e| e.into_inner());

        checksum.update_slice(
            &value_type_to_u8(guard.global_type_descriptor().value_type).to_le_bytes(),
        );
        checksum.update_slice(&u8::from(guard.global_type_descriptor().mutable).to_le_bytes());
    }
}

impl ToBytes for GlobalWrapper {
    fn serialized_size(&self) -> usize {
        20 // value type (1) + mutable flag (1) + padding (2) + value (16 bytes for v128 compat)
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream,
        _provider: &P,
    ) -> Result<()> {
        let guard = self.0.read().map_err(|_| {
            kiln_error::Error::runtime_execution_error("Failed to acquire read lock on global")
        })?;

        // Write value type (1 byte)
        writer.write_u8(value_type_to_u8(guard.global_type_descriptor().value_type))?;

        // Write mutable flag (1 byte)
        writer.write_u8(if guard.global_type_descriptor().mutable { 1 } else { 0 })?;

        // Write padding (2 bytes)
        writer.write_u8(0)?;
        writer.write_u8(0)?;

        // Write value (16 bytes - v128 size for maximum compatibility)
        let value = guard.get();
        match value {
            KilnValue::I32(v) => {
                writer.write_all(&(*v as u32).to_le_bytes())?;
                writer.write_all(&[0u8; 12])?;
            },
            KilnValue::I64(v) => {
                writer.write_all(&(*v as u64).to_le_bytes())?;
                writer.write_all(&[0u8; 8])?;
            },
            KilnValue::F32(kiln_foundation::values::FloatBits32(bits)) => {
                writer.write_all(&bits.to_le_bytes())?;
                writer.write_all(&[0u8; 12])?;
            },
            KilnValue::F64(kiln_foundation::values::FloatBits64(bits)) => {
                writer.write_all(&bits.to_le_bytes())?;
                writer.write_all(&[0u8; 8])?;
            },
            KilnValue::V128(v) => {
                writer.write_all(&v.bytes)?;
            },
            KilnValue::FuncRef(ref_opt) => {
                let value = match ref_opt {
                    Some(func_ref) => func_ref.index,
                    None => 0xFFFFFFFF,
                };
                writer.write_all(&value.to_le_bytes())?;
                writer.write_all(&[0u8; 12])?;
            },
            KilnValue::ExternRef(ref_opt) => {
                let value = match ref_opt {
                    Some(extern_ref) => extern_ref.index,
                    None => 0xFFFFFFFF,
                };
                writer.write_all(&value.to_le_bytes())?;
                writer.write_all(&[0u8; 12])?;
            },
            KilnValue::ExnRef(ref_opt) => {
                let value = match ref_opt {
                    Some(exn_ref) => *exn_ref as u32,
                    None => 0xFFFFFFFF,
                };
                writer.write_all(&value.to_le_bytes())?;
                writer.write_all(&[0u8; 12])?;
            },
            KilnValue::I31Ref(ref_opt) => {
                let value = match ref_opt {
                    Some(i31_ref) => *i31_ref as u32,
                    None => 0xFFFFFFFF,
                };
                writer.write_all(&value.to_le_bytes())?;
                writer.write_all(&[0u8; 12])?;
            },
            KilnValue::StructRef(ref_opt) => {
                let value = match ref_opt {
                    Some(struct_ref) => struct_ref.type_index(),
                    None => 0xFFFFFFFF,
                };
                writer.write_all(&value.to_le_bytes())?;
                writer.write_all(&[0u8; 12])?;
            },
            KilnValue::ArrayRef(ref_opt) => {
                let value = match ref_opt {
                    Some(array_ref) => array_ref.type_index(),
                    None => 0xFFFFFFFF,
                };
                writer.write_all(&value.to_le_bytes())?;
                writer.write_all(&[0u8; 12])?;
            },
            _ => {
                // For other types, write zeros (16 bytes)
                writer.write_all(&[0u8; 16])?;
            }
        }
        Ok(())
    }
}

impl FromBytes for GlobalWrapper {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        // Deserialize the global data properly
        use kiln_foundation::{
            types::ValueType,
            values::Value,
        };

        // Read value type (1 byte)
        let value_type_byte = reader.read_u8()?;
        let value_type = match value_type_byte {
            0 => ValueType::I32,
            1 => ValueType::I64,
            2 => ValueType::F32,
            3 => ValueType::F64,
            4 => ValueType::FuncRef,
            5 => ValueType::ExternRef,
            6 => ValueType::V128,
            8 => ValueType::StructRef(0), // placeholder type index
            9 => ValueType::ArrayRef(0),  // placeholder type index
            10 => ValueType::ExnRef,
            11 => ValueType::I31Ref,
            12 => ValueType::AnyRef,
            13 => ValueType::EqRef,
            _ => ValueType::I32, // Default fallback
        };

        // Read mutable flag (1 byte)
        let mutable = reader.read_u8()? != 0;

        // Read padding (2 bytes to align to 4)
        let _ = reader.read_u8()?;
        let _ = reader.read_u8()?;

        // Read value (16 bytes - v128 size for maximum compatibility)
        let mut value_bytes = [0u8; 16];
        for i in 0..16 {
            value_bytes[i] = reader.read_u8()?;
        }

        let value_low = u32::from_le_bytes([value_bytes[0], value_bytes[1], value_bytes[2], value_bytes[3]]);
        let value_high = u32::from_le_bytes([value_bytes[4], value_bytes[5], value_bytes[6], value_bytes[7]]);

        let value = match value_type {
            ValueType::I32 => Value::I32(value_low as i32),
            ValueType::I64 => {
                let v = ((value_high as i64) << 32) | (value_low as i64);
                Value::I64(v)
            },
            ValueType::F32 => Value::F32(kiln_foundation::values::FloatBits32(value_low)),
            ValueType::F64 => {
                let v = ((value_high as u64) << 32) | (value_low as u64);
                Value::F64(kiln_foundation::values::FloatBits64(v))
            },
            ValueType::V128 => {
                Value::V128(kiln_foundation::values::V128 { bytes: value_bytes })
            },
            ValueType::FuncRef => {
                // 0xFFFFFFFF means None, otherwise it's an index
                if value_low == 0xFFFFFFFF {
                    Value::FuncRef(None)
                } else {
                    Value::FuncRef(Some(kiln_foundation::values::FuncRef::from_index(value_low)))
                }
            },
            ValueType::ExternRef => {
                // 0xFFFFFFFF means None, otherwise it's an index
                if value_low == 0xFFFFFFFF {
                    Value::ExternRef(None)
                } else {
                    Value::ExternRef(Some(kiln_foundation::values::ExternRef { index: value_low }))
                }
            },
            ValueType::ExnRef => {
                // 0xFFFFFFFF means None, otherwise it's an index
                // ExnRef in Value is Option<u32>
                if value_low == 0xFFFFFFFF {
                    Value::ExnRef(None)
                } else {
                    Value::ExnRef(Some(value_low))
                }
            },
            ValueType::I31Ref => {
                // 0xFFFFFFFF means None, otherwise it's a value
                // I31Ref in Value is Option<i32>
                if value_low == 0xFFFFFFFF {
                    Value::I31Ref(None)
                } else {
                    Value::I31Ref(Some(value_low as i32))
                }
            },
            ValueType::StructRef(_) | ValueType::ArrayRef(_) => {
                // StructRef/ArrayRef are complex types - for null values, just use None
                // For non-null, we'd need full struct data which isn't available in serialization
                if value_low == 0xFFFFFFFF {
                    Value::StructRef(None) // Both use None for null
                } else {
                    // Can't reconstruct full struct/array from just an index
                    Value::StructRef(None)
                }
            },
            ValueType::AnyRef | ValueType::EqRef => {
                // AnyRef/EqRef: use ExternRef representation for null
                if value_low == 0xFFFFFFFF {
                    Value::ExternRef(None)
                } else {
                    Value::ExternRef(Some(kiln_foundation::values::ExternRef { index: value_low }))
                }
            },
            _ => Value::I32(value_low as i32),
        };

        let global = Global::new(value_type, mutable, value).map_err(|_| {
            kiln_error::Error::runtime_execution_error("Failed to create global from bytes")
        })?;

        Ok(GlobalWrapper::new(global))
    }
}

// Arc<Table> trait implementations removed due to orphan rule violations.
// Use TableWrapper instead which implements these traits properly.

// Trait implementations for Arc<Memory>

// Default for Arc<Memory> removed due to orphan rules - use explicit creation
// instead
//

// Arc<Memory> trait implementations removed due to orphan rule violations.
// Use MemoryWrapper instead which implements these traits properly.

// Trait implementations for Arc<Global>

// Default for Arc<Global> removed due to orphan rules - use explicit creation
// instead

// Arc<Global> trait implementations removed due to orphan rule violations.
// Use GlobalWrapper instead which implements these traits properly.

// Ensure local `crate::module::Import` struct is defined
// Ensure local `crate::module::Export` struct is defined
// Ensure local `crate::global::Global`, `crate::table::Table`,
// `crate::memory::Memory` are defined and their `new` methods are compatible.
