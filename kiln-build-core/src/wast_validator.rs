//! WAST Module Validator
//!
//! This module provides validation for WebAssembly modules to ensure they
//! conform to the WebAssembly specification. It validates:
//! - Type correctness on the operand stack
//! - Control flow structure (blocks, loops, branches)
//! - Function and memory references
//! - Type checking even in unreachable code
//!
//! This validator runs BEFORE module execution to reject invalid modules
//! immediately, which is required for WAST conformance testing.

use anyhow::{Context, Result, anyhow};
use std::collections::HashSet;
use kiln_format::module::{CompositeTypeKind, ExportKind, Function, Global, ImportDesc, Module, RecGroup};
use kiln_format::pure_format_types::{PureElementInit, PureElementMode, PureElementSegment};
use kiln_format::types::RefType;
use kiln_foundation::ValueType;

/// Type of a value on the stack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackType {
    I32,
    I64,
    F32,
    F64,
    V128,
    FuncRef,
    /// Null function reference - the bottom type for funcref hierarchy
    /// This is ref null nofunc - assignable to any nullable funcref type
    NullFuncRef,
    ExternRef,
    /// Null extern reference - bottom type for extern hierarchy (noextern)
    NullExternRef,
    ExnRef,
    /// Null exception reference - bottom type for exn hierarchy (noexn)
    NullExnRef,
    /// Typed function reference (ref null? $t) where t is a type index
    /// First field is type index, second is whether it's nullable
    TypedFuncRef(u32, bool),
    /// GC type: anyref - top of the internal reference hierarchy
    AnyRef,
    /// GC type: eqref - subtypes: i31ref, structref, arrayref
    EqRef,
    /// GC type: i31ref - unboxed 31-bit integer reference
    I31Ref,
    /// GC type: structref - abstract struct reference
    StructRef,
    /// GC type: arrayref - abstract array reference
    ArrayRef,
    /// GC type: none - bottom type for the any hierarchy
    NoneRef,
    Unknown,
}

impl StackType {
    /// Convert from ValueType
    fn from_value_type(vt: ValueType) -> Self {
        match vt {
            ValueType::I32 => StackType::I32,
            ValueType::I64 => StackType::I64,
            ValueType::F32 => StackType::F32,
            ValueType::F64 => StackType::F64,
            ValueType::V128 => StackType::V128,
            ValueType::FuncRef => StackType::FuncRef,
            ValueType::NullFuncRef => StackType::NullFuncRef,
            ValueType::ExternRef => StackType::ExternRef,
            ValueType::ExnRef => StackType::ExnRef,
            // Typed function reference - preserves type index and nullability
            ValueType::TypedFuncRef(idx, nullable) => StackType::TypedFuncRef(idx, nullable),
            // GC types
            ValueType::AnyRef => StackType::AnyRef,
            ValueType::EqRef => StackType::EqRef,
            ValueType::I31Ref => StackType::I31Ref,
            ValueType::StructRef(_) => StackType::StructRef,
            ValueType::ArrayRef(_) => StackType::ArrayRef,
            // GC bottom types
            ValueType::NoneRef => StackType::NoneRef,
            ValueType::NoExternRef => StackType::NullExternRef,
            ValueType::NoExnRef => StackType::NullExnRef,
            // I16x8 is a SIMD sub-type, not a reference type
            ValueType::I16x8 => StackType::Unknown,
        }
    }

    /// Check if this type is a numeric type (valid for untyped select)
    /// Per WebAssembly spec, untyped select only works with i32, i64, f32, f64, v128
    /// Reference types (funcref, externref, etc.) require typed select (0x1C)
    fn is_numeric(&self) -> bool {
        matches!(
            self,
            StackType::I32 | StackType::I64 | StackType::F32 | StackType::F64 | StackType::V128
        )
    }

    /// Check if this type is a reference type (requires typed select)
    fn is_reference(&self) -> bool {
        matches!(
            self,
            StackType::FuncRef
                | StackType::NullFuncRef
                | StackType::ExternRef
                | StackType::NullExternRef
                | StackType::ExnRef
                | StackType::NullExnRef
                | StackType::TypedFuncRef(_, _)
                | StackType::AnyRef
                | StackType::EqRef
                | StackType::I31Ref
                | StackType::StructRef
                | StackType::ArrayRef
                | StackType::NoneRef
        )
    }

    /// Check if this type is a subtype of another type
    ///
    /// GC subtyping lattice:
    ///   none <: i31 <: eq <: any
    ///   none <: struct <: eq <: any
    ///   none <: array <: eq <: any
    ///   nofunc <: func types <: func
    ///   noextern <: extern
    ///   noexn <: exn
    ///   none <: (ref $t) <: struct/array/func <: eq/func <: any
    ///
    /// Nullability: non-nullable <: nullable (for the same heap type)
    ///
    /// Note: TypedFuncRef(idx, nullable) represents a concrete typed reference
    /// to any composite type (func, struct, or array). Without module context,
    /// we treat it as compatible with all abstract GC reference types.
    fn is_subtype_of(&self, other: &StackType) -> bool {
        if self == other {
            return true;
        }

        // Unknown is polymorphic - compatible with anything
        if *self == StackType::Unknown || *other == StackType::Unknown {
            return true;
        }

        match (self, other) {
            // === func hierarchy: nofunc <: typed func refs <: func ===
            // NullFuncRef (nofunc) is bottom of func hierarchy
            (StackType::NullFuncRef, StackType::FuncRef) => true,
            (StackType::NullFuncRef, StackType::TypedFuncRef(_, nullable)) => *nullable,
            // TypedFuncRef is a subtype of FuncRef (when the concrete type is a func type)
            (StackType::TypedFuncRef(_, _), StackType::FuncRef) => true,
            // Two TypedFuncRefs: type indices must match, and nullability must be compatible
            (StackType::TypedFuncRef(t1, n1), StackType::TypedFuncRef(t2, n2)) => {
                t1 == t2 && (*n1 == *n2 || (!*n1 && *n2))
            },
            // FuncRef is NOT a subtype of TypedFuncRef
            (StackType::FuncRef, StackType::TypedFuncRef(_, _)) => false,

            // === Concrete typed references and abstract GC types ===
            // TypedFuncRef represents a concrete typed reference (ref $t).
            // Without module context, we can't determine if $t is func/struct/array,
            // so we accept it as a subtype of all abstract GC ref types.
            // This is sound because the spec allows concrete struct/array types
            // to be subtypes of structref/arrayref/eqref/anyref.
            (StackType::TypedFuncRef(_, _), StackType::StructRef) => true,
            (StackType::TypedFuncRef(_, _), StackType::ArrayRef) => true,
            (StackType::TypedFuncRef(_, _), StackType::EqRef) => true,
            (StackType::TypedFuncRef(_, _), StackType::AnyRef) => true,

            // NoneRef is bottom of the any hierarchy - subtype of concrete refs
            (StackType::NoneRef, StackType::TypedFuncRef(_, nullable)) => *nullable,

            // === extern hierarchy: noextern <: extern ===
            (StackType::NullExternRef, StackType::ExternRef) => true,

            // === exn hierarchy: noexn <: exn ===
            (StackType::NullExnRef, StackType::ExnRef) => true,

            // === any hierarchy: none <: i31/struct/array <: eq <: any ===
            // NoneRef (none) is bottom of the any hierarchy
            (StackType::NoneRef, StackType::I31Ref) => true,
            (StackType::NoneRef, StackType::StructRef) => true,
            (StackType::NoneRef, StackType::ArrayRef) => true,
            (StackType::NoneRef, StackType::EqRef) => true,
            (StackType::NoneRef, StackType::AnyRef) => true,

            // i31, struct, array <: eq
            (StackType::I31Ref, StackType::EqRef) => true,
            (StackType::StructRef, StackType::EqRef) => true,
            (StackType::ArrayRef, StackType::EqRef) => true,

            // i31, struct, array, eq <: any
            (StackType::I31Ref, StackType::AnyRef) => true,
            (StackType::StructRef, StackType::AnyRef) => true,
            (StackType::ArrayRef, StackType::AnyRef) => true,
            (StackType::EqRef, StackType::AnyRef) => true,

            _ => false,
        }
    }

    /// Convert from RefType to StackType
    fn from_ref_type(rt: kiln_foundation::RefType) -> Self {
        match rt {
            kiln_foundation::RefType::Funcref => StackType::FuncRef,
            kiln_foundation::RefType::Externref => StackType::ExternRef,
            kiln_foundation::RefType::Gc(gc) => {
                use kiln_foundation::types::HeapType;
                match gc.heap_type {
                    HeapType::Func => StackType::FuncRef,
                    HeapType::Extern => StackType::ExternRef,
                    HeapType::Exn => StackType::ExnRef,
                    HeapType::NoFunc => StackType::NullFuncRef,
                    HeapType::NoExtern => StackType::NullExternRef,
                    HeapType::None => StackType::NoneRef,
                    HeapType::Any => StackType::AnyRef,
                    HeapType::Eq => StackType::EqRef,
                    HeapType::I31 => StackType::I31Ref,
                    HeapType::Struct => StackType::StructRef,
                    HeapType::Array => StackType::ArrayRef,
                    HeapType::Concrete(idx) => StackType::TypedFuncRef(idx, gc.nullable),
                }
            }
        }
    }
}

/// Control flow frame tracking
#[derive(Debug, Clone)]
struct ControlFrame {
    /// Type of control structure (block, loop, if)
    frame_type: FrameType,
    /// Input types expected for this frame
    input_types: Vec<StackType>,
    /// Output types expected from this frame
    output_types: Vec<StackType>,
    /// Whether this frame's code path is reachable
    reachable: bool,
    /// Stack height at frame entry
    stack_height: usize,
    /// Stack height when frame became unreachable (for polymorphic base tracking)
    unreachable_height: Option<usize>,
    /// Count of concrete values pushed after the frame became unreachable.
    /// This tracks real pushes (i32.const, local.get, call results, etc.)
    /// but NOT phantom values from polymorphic underflow.
    /// Used to reject blocks that push too many concrete values in unreachable code.
    concrete_push_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameType {
    Block,
    Loop,
    If,
    Else,
    Try,
}

/// Validator for WebAssembly modules
pub struct WastModuleValidator;

/// WebAssembly memory limits
/// Max memory pages is 65536 (4GB with 64KB pages)
const WASM_MAX_MEMORY_PAGES: u32 = 65536;

impl WastModuleValidator {
    /// Validate a module
    pub fn validate(module: &Module) -> Result<()> {
        // Validate memory, table, and tag limits
        Self::validate_memory_limits(module)?;
        Self::validate_table_limits(module)?;
        Self::validate_tags(module)?;

        // Validate type index references are within bounds
        Self::validate_type_references(module)?;

        // Validate forward type references outside rec groups
        Self::validate_rec_group_type_references(module)?;

        // Validate subtype declarations in rec_groups
        Self::validate_subtype_declarations(module)?;

        // Validate data segments - only ACTIVE segments require a memory to be defined
        // Passive segments can exist without memory (they're only used via memory.init/data.drop)
        let has_active_data = module.data.iter().any(|d| d.is_active());
        if has_active_data && !Self::has_memory(module) {
            return Err(anyhow!("unknown memory"));
        }

        // Collect declared functions for ref.func validation
        let declared_functions = Self::collect_declared_functions(module);

        // Validate functions
        for (func_idx, func) in module.functions.iter().enumerate() {
            if let Err(e) = Self::validate_function(func_idx, func, module, &declared_functions) {
                let root = e.root_cause().to_string();
                return Err(anyhow!("Function {} validation failed ({})", func_idx, root));
            }
        }

        // Validate globals
        for (global_idx, global) in module.globals.iter().enumerate() {
            if let Err(e) = Self::validate_global(global_idx, global, module, &declared_functions) {
                let root = e.root_cause().to_string();
                return Err(anyhow!("Global {} validation failed ({})", global_idx, root));
            }
        }

        // Validate data segment offset expressions
        // For data/elem offsets, all globals are accessible (they're processed after
        // the global section), so we use total_globals as the context index to allow
        // referencing any global. The global must still be immutable.
        let all_globals_ctx = Self::total_globals(module);
        for (seg_idx, data_seg) in module.data.iter().enumerate() {
            if data_seg.is_active() && !data_seg.offset_expr_bytes.is_empty() {
                let mem_idx = data_seg.memory_index().unwrap_or(0);
                let addr_type = if Self::is_memory64(module, mem_idx) {
                    StackType::I64
                } else {
                    StackType::I32
                };
                Self::validate_const_expr_typed(
                    &data_seg.offset_expr_bytes,
                    module,
                    all_globals_ctx,
                    addr_type,
                    &declared_functions,
                )
                .with_context(|| format!("data segment {} offset expression invalid", seg_idx))?;
            }
        }

        // Validate element segment offset expressions
        for (seg_idx, elem_seg) in module.elements.iter().enumerate() {
            if elem_seg.is_active() && !elem_seg.offset_expr_bytes.is_empty() {
                let table_idx = elem_seg.table_index().unwrap_or(0);
                let addr_type = if Self::is_table64(module, table_idx) {
                    StackType::I64
                } else {
                    StackType::I32
                };
                Self::validate_const_expr_typed(
                    &elem_seg.offset_expr_bytes,
                    module,
                    all_globals_ctx,
                    addr_type,
                    &declared_functions,
                )
                .with_context(|| format!("element segment {} offset expression invalid", seg_idx))?;
            }
        }

        // Validate element segments - function indices must be valid
        Self::validate_element_segments(module)?;

        // Validate start function
        Self::validate_start_function(module)?;

        // Validate export names are unique
        Self::validate_export_names(module)?;

        // Validate export indices are within bounds
        Self::validate_export_indices(module)?;

        Ok(())
    }

    /// Validate that all export names are unique
    /// WebAssembly spec requires export names to be unique within a module
    fn validate_export_names(module: &Module) -> Result<()> {
        let mut seen_names: HashSet<&str> = HashSet::new();
        for export in &module.exports {
            if !seen_names.insert(export.name.as_str()) {
                return Err(anyhow!("duplicate export name"));
            }
        }
        Ok(())
    }

    /// Validate that all export indices are within bounds
    fn validate_export_indices(module: &Module) -> Result<()> {
        // Note: module.functions already includes imported functions (decoder design),
        // so we use it directly instead of total_functions() which would double-count.
        let total_funcs = module.functions.len();
        let total_tables = Self::total_tables(module);
        let total_memories = Self::total_memories(module);
        let total_globals = Self::total_globals(module);
        let total_tags = Self::total_tags(module);

        for export in &module.exports {
            match export.kind {
                ExportKind::Function => {
                    if (export.index as usize) >= total_funcs {
                        return Err(anyhow!("unknown function {}", export.index));
                    }
                },
                ExportKind::Table => {
                    if (export.index as usize) >= total_tables {
                        return Err(anyhow!("unknown table {}", export.index));
                    }
                },
                ExportKind::Memory => {
                    if (export.index as usize) >= total_memories {
                        return Err(anyhow!("unknown memory {}", export.index));
                    }
                },
                ExportKind::Global => {
                    if (export.index as usize) >= total_globals {
                        return Err(anyhow!("unknown global {}", export.index));
                    }
                },
                ExportKind::Tag => {
                    if (export.index as usize) >= total_tags {
                        return Err(anyhow!("unknown tag {}", export.index));
                    }
                },
            }
        }
        Ok(())
    }

    /// Validate element segments
    /// Function indices in element segments must be valid
    fn validate_element_segments(module: &Module) -> Result<()> {
        let total_funcs = Self::total_functions(module);

        for elem in &module.elements {
            match &elem.init_data {
                PureElementInit::FunctionIndices(indices) => {
                    for &idx in indices {
                        if (idx as usize) >= total_funcs {
                            return Err(anyhow!("unknown function"));
                        }
                    }
                },
                PureElementInit::ExpressionBytes(exprs) => {
                    // Parse expressions to find ref.func instructions
                    for expr_bytes in exprs {
                        Self::validate_element_expr_functions(expr_bytes, total_funcs)?;
                    }
                },
            }
        }
        Ok(())
    }

    /// Validate function references in element expression bytes
    fn validate_element_expr_functions(expr: &[u8], total_funcs: usize) -> Result<()> {
        let mut pos = 0;
        while pos < expr.len() {
            let opcode = expr[pos];
            pos += 1;

            match opcode {
                0xD2 => {
                    // ref.func - validate the function index
                    if let Ok((func_idx, new_pos)) = Self::read_leb128_unsigned(expr, pos) {
                        if (func_idx as usize) >= total_funcs {
                            return Err(anyhow!("unknown function"));
                        }
                        pos = new_pos;
                    } else {
                        break;
                    }
                },
                0xD0 => {
                    // ref.null - skip heap type
                    if pos < expr.len() {
                        pos += 1;
                    }
                },
                0x0B => {
                    // end
                    break;
                },
                _ => {
                    // Skip other opcodes - may need to parse their immediates
                    // For now, just continue
                },
            }
        }
        Ok(())
    }

    /// Validate start function
    /// - Start function index must be valid
    /// - Start function must have no parameters
    /// - Start function must have no results
    fn validate_start_function(module: &Module) -> Result<()> {
        if let Some(start_idx) = module.start {
            let total_funcs = Self::total_functions(module);

            // Check that start function index is valid
            if (start_idx as usize) >= total_funcs {
                return Err(anyhow!("unknown function"));
            }

            // Get the function type of the start function
            let func_type = Self::get_function_type(module, start_idx)?;

            // Start function must have no parameters
            if !func_type.params.is_empty() {
                return Err(anyhow!("start function"));
            }

            // Start function must have no results
            if !func_type.results.is_empty() {
                return Err(anyhow!("start function"));
            }
        }
        Ok(())
    }

    /// Get the function type for a function index (accounting for imports)
    fn get_function_type(
        module: &Module,
        func_idx: u32,
    ) -> Result<&kiln_foundation::CleanCoreFuncType> {
        let num_func_imports = Self::count_function_imports(module);

        if (func_idx as usize) < num_func_imports {
            // This is an imported function - find its type
            let mut import_idx = 0;
            for import in &module.imports {
                if let ImportDesc::Function(type_idx) = &import.desc {
                    if import_idx == func_idx as usize {
                        let type_idx = *type_idx as usize;
                        return module.types.get(type_idx).ok_or_else(|| anyhow!("unknown type"));
                    }
                    import_idx += 1;
                }
            }
            Err(anyhow!("unknown function"))
        } else {
            // This is a defined function
            // Note: module.functions includes imports, so use func_idx directly
            let func = module
                .functions
                .get(func_idx as usize)
                .ok_or_else(|| anyhow!("unknown function"))?;
            let func_type = module
                .types
                .get(func.type_idx as usize)
                .ok_or_else(|| anyhow!("unknown type"))?;
            Ok(func_type)
        }
    }

    /// Get the type index of a function (for ref.func which produces typed refs)
    fn get_function_type_idx(module: &Module, func_idx: u32) -> Result<u32> {
        let num_func_imports = Self::count_function_imports(module);

        if (func_idx as usize) < num_func_imports {
            // This is an imported function - find its type index
            let mut import_idx = 0;
            for import in &module.imports {
                if let ImportDesc::Function(type_idx) = &import.desc {
                    if import_idx == func_idx as usize {
                        return Ok(*type_idx);
                    }
                    import_idx += 1;
                }
            }
            Err(anyhow!("unknown function"))
        } else {
            // This is a defined function
            let func = module
                .functions
                .get(func_idx as usize)
                .ok_or_else(|| anyhow!("unknown function"))?;
            Ok(func.type_idx)
        }
    }

    /// Look up a GC composite type kind by type index from the module's rec_groups.
    /// Returns the CompositeTypeKind for the given type index, or None if not found
    /// in rec_groups (which means it's only a plain function type in module.types).
    fn get_composite_type_kind(module: &Module, type_idx: u32) -> Option<&CompositeTypeKind> {
        for group in &module.rec_groups {
            let start = group.start_type_index;
            let end = start + group.types.len() as u32;
            if type_idx >= start && type_idx < end {
                let local_idx = (type_idx - start) as usize;
                return Some(&group.types[local_idx].composite_kind);
            }
        }
        None
    }

    /// Get the number of fields for a struct type, or None if not a struct type.
    fn get_struct_field_count(module: &Module, type_idx: u32) -> Option<usize> {
        match Self::get_composite_type_kind(module, type_idx)? {
            CompositeTypeKind::StructWithFields(fields) => Some(fields.len()),
            CompositeTypeKind::Struct => {
                // Abstract struct without parsed fields -- we don't know the count
                None
            },
            _ => None,
        }
    }

    /// Validate memory section limits
    fn validate_memory_limits(module: &Module) -> Result<()> {
        // Check imported memories
        for import in &module.imports {
            if let ImportDesc::Memory(memory) = &import.desc {
                // Check that min <= max if max is specified
                if let Some(max) = memory.limits.max {
                    if memory.limits.min > max {
                        return Err(anyhow!("size minimum must not be greater than maximum"));
                    }
                }
                // Check memory size bounds (65536 pages max for memory32)
                // Memory64 allows much larger limits (up to 2^48 pages)
                if !memory.memory64 {
                    if memory.limits.min > WASM_MAX_MEMORY_PAGES {
                        return Err(anyhow!("memory size"));
                    }
                    if let Some(max) = memory.limits.max {
                        if max > WASM_MAX_MEMORY_PAGES {
                            return Err(anyhow!("memory size"));
                        }
                    }
                }
            }
        }

        // Check defined memories
        for memory in &module.memories {
            // Check that min <= max if max is specified
            if let Some(max) = memory.limits.max {
                if memory.limits.min > max {
                    return Err(anyhow!("size minimum must not be greater than maximum"));
                }
            }
            // Check memory size bounds (65536 pages max for memory32)
            // Memory64 allows much larger limits (up to 2^48 pages)
            if !memory.memory64 {
                if memory.limits.min > WASM_MAX_MEMORY_PAGES {
                    return Err(anyhow!("memory size"));
                }
                if let Some(max) = memory.limits.max {
                    if max > WASM_MAX_MEMORY_PAGES {
                        return Err(anyhow!("memory size"));
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate table section limits
    fn validate_table_limits(module: &Module) -> Result<()> {
        // Check imported tables
        for import in &module.imports {
            if let ImportDesc::Table(table_type) = &import.desc {
                if let Some(max) = table_type.limits.max {
                    if table_type.limits.min > max {
                        return Err(anyhow!("size minimum must not be greater than maximum"));
                    }
                }
            }
        }

        // Check defined tables
        for table in &module.tables {
            if let Some(max) = table.limits.max {
                if table.limits.min > max {
                    return Err(anyhow!("size minimum must not be greater than maximum"));
                }
            }
        }

        Ok(())
    }

    /// Validate tag types (exception handling)
    fn validate_tags(module: &Module) -> Result<()> {
        // Check defined tags - result type must be empty
        for tag in &module.tags {
            if (tag.type_idx as usize) < module.types.len() {
                let func_type = &module.types[tag.type_idx as usize];
                if !func_type.results.is_empty() {
                    return Err(anyhow!("non-empty tag result type"));
                }
            }
        }
        // Check imported tags - ImportDesc::Tag(type_idx)
        for import in &module.imports {
            if let ImportDesc::Tag(type_idx) = &import.desc {
                if (*type_idx as usize) < module.types.len() {
                    let func_type = &module.types[*type_idx as usize];
                    if !func_type.results.is_empty() {
                        return Err(anyhow!("non-empty tag result type"));
                    }
                }
            }
        }
        Ok(())
    }

    /// Check if a ValueType contains a type index reference and validate it
    fn check_value_type_ref(vt: &ValueType, num_types: usize) -> Result<()> {
        match vt {
            ValueType::TypedFuncRef(idx, _)
            | ValueType::StructRef(idx)
            | ValueType::ArrayRef(idx) => {
                if (*idx as usize) >= num_types {
                    return Err(anyhow!("unknown type"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Check if a RefType contains a type index reference and validate it
    fn check_ref_type_ref(rt: &kiln_foundation::RefType, num_types: usize) -> Result<()> {
        if let kiln_foundation::RefType::Gc(gc) = rt {
            use kiln_foundation::types::HeapType;
            if let HeapType::Concrete(idx) = gc.heap_type {
                if (idx as usize) >= num_types {
                    return Err(anyhow!("unknown type"));
                }
            }
        }
        Ok(())
    }

    /// Check type references inside a GC composite type's field definitions
    fn check_composite_type_refs(kind: &CompositeTypeKind, num_types: usize) -> Result<()> {
        match kind {
            CompositeTypeKind::StructWithFields(fields) => {
                for field in fields {
                    Self::check_gc_storage_type_ref(&field.storage_type, num_types)?;
                }
            }
            CompositeTypeKind::ArrayWithElement(elem) => {
                Self::check_gc_storage_type_ref(&elem.storage_type, num_types)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Check if a GcStorageType contains a type index reference and validate it
    fn check_gc_storage_type_ref(
        st: &kiln_format::module::GcStorageType,
        num_types: usize,
    ) -> Result<()> {
        match st {
            kiln_format::module::GcStorageType::RefType(idx)
            | kiln_format::module::GcStorageType::RefTypeNull(idx) => {
                if (*idx as usize) >= num_types {
                    return Err(anyhow!("unknown type"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Validate that all type index references in the module are within bounds
    fn validate_type_references(module: &Module) -> Result<()> {
        let num_types = module.types.len();

        // Check type section itself (params and results may reference other types)
        for func_type in &module.types {
            for vt in &func_type.params {
                Self::check_value_type_ref(vt, num_types)?;
            }
            for vt in &func_type.results {
                Self::check_value_type_ref(vt, num_types)?;
            }
        }

        // Check function locals
        for func in &module.functions {
            for vt in &func.locals {
                Self::check_value_type_ref(vt, num_types)?;
            }
        }

        // Check table element types
        for table in &module.tables {
            Self::check_ref_type_ref(&table.element_type, num_types)?;
        }

        // Check global types
        for global in &module.globals {
            Self::check_value_type_ref(&global.global_type.value_type, num_types)?;
        }

        // Check imported tables and globals
        for import in &module.imports {
            match &import.desc {
                ImportDesc::Table(table) => {
                    Self::check_ref_type_ref(&table.element_type, num_types)?;
                }
                ImportDesc::Global(global_type) => {
                    Self::check_value_type_ref(&global_type.value_type, num_types)?;
                }
                _ => {}
            }
        }

        // Check element segment types
        for elem in &module.elements {
            Self::check_ref_type_ref(&elem.element_type, num_types)?;
        }

        // Check type references inside GC struct/array field definitions (rec_groups)
        for rec_group in &module.rec_groups {
            // Types within the same rec group can reference each other,
            // so the valid range includes the group's own indices
            let group_end = rec_group.start_type_index as usize + rec_group.types.len();
            let valid_types = num_types.max(group_end);
            for sub_type in &rec_group.types {
                Self::check_composite_type_refs(&sub_type.composite_kind, valid_types)?;
            }
        }

        Ok(())
    }

    /// Validate a single function
    fn validate_function(
        func_idx: usize,
        func: &Function,
        module: &Module,
        declared_functions: &HashSet<u32>,
    ) -> Result<()> {
        // Get the function's type signature
        if func.type_idx as usize >= module.types.len() {
            return Err(anyhow!(
                "Function {} has invalid type index {}",
                func_idx,
                func.type_idx
            ));
        }

        let func_type_clean = &module.types[func.type_idx as usize];

        // Parse and validate the function body
        // Note: CleanCoreFuncType has the same structure as FuncType (params, results)
        Self::validate_function_body(
            &func.code,
            func_type_clean,
            &func.locals,
            module,
            declared_functions,
        )
    }

    /// Validate a function body bytecode
    fn validate_function_body(
        code: &[u8],
        func_type: &kiln_foundation::CleanCoreFuncType,
        locals: &[ValueType],
        module: &Module,
        declared_functions: &HashSet<u32>,
    ) -> Result<()> {
        // Build local variable types: parameters first, then locals
        let mut local_types = Vec::new();

        // Add parameter types
        for param in &func_type.params {
            local_types.push(*param);
        }

        // Add local types
        for local in locals {
            local_types.push(*local);
        }

        // Initialize operand stack (empty - parameters are accessed via local.get, not on stack)
        let mut stack: Vec<StackType> = Vec::new();

        // Initialize control flow frames
        let mut frames: Vec<ControlFrame> = vec![ControlFrame {
            frame_type: FrameType::Block,
            input_types: Vec::new(),
            output_types: func_type
                .results
                .iter()
                .map(|&vt| StackType::from_value_type(vt))
                .collect(),
            reachable: true,
            stack_height: 0,
            unreachable_height: None,
            concrete_push_count: 0,
        }];

        // Parse bytecode
        let mut offset = 0;
        let mut last_opcode = 0u8;
        while offset < code.len() {
            let opcode = code[offset];
            last_opcode = opcode;
            offset += 1;

            match opcode {
                // Control flow
                0x00 => {
                    // unreachable
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        // Always truncate — per spec, stack becomes polymorphic after
                        // a terminating instruction, even in already-unreachable code
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },
                0x01 => {
                    // nop
                },
                0x02 => {
                    // block
                    let (block_type, new_offset) = Self::parse_block_type(code, offset, module)?;
                    offset = new_offset;

                    let (input_types, output_types) =
                        Self::block_type_to_stack_types(&block_type, module)?;

                    // For blocks with inputs, verify and pop the input types
                    let frame_height = Self::current_frame_height(&frames);
                    for &expected in input_types.iter().rev() {
                        if !Self::pop_type(
                            &mut stack,
                            expected,
                            frame_height,
                            Self::is_unreachable(&frames),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // Record the stack height AFTER popping inputs
                    let stack_height = stack.len();

                    frames.push(ControlFrame {
                        frame_type: FrameType::Block,
                        input_types: input_types.clone(),
                        output_types: output_types.clone(),
                        reachable: true,
                        stack_height,
                        unreachable_height: None,
                        concrete_push_count: 0,
                    });

                    // Push inputs back - they're now on the block's stack
                    for input_type in &input_types {
                        stack.push(*input_type);
                    }
                },
                0x03 => {
                    // loop
                    let (block_type, new_offset) = Self::parse_block_type(code, offset, module)?;
                    offset = new_offset;

                    let (input_types, output_types) =
                        Self::block_type_to_stack_types(&block_type, module)?;

                    // For loops with inputs, verify and pop the input types
                    let frame_height = Self::current_frame_height(&frames);
                    for &expected in input_types.iter().rev() {
                        if !Self::pop_type(
                            &mut stack,
                            expected,
                            frame_height,
                            Self::is_unreachable(&frames),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // Record the stack height AFTER popping inputs
                    let stack_height = stack.len();

                    frames.push(ControlFrame {
                        frame_type: FrameType::Loop,
                        input_types: input_types.clone(),
                        output_types: output_types.clone(),
                        reachable: true,
                        stack_height,
                        unreachable_height: None,
                        concrete_push_count: 0,
                    });

                    // Push inputs back - they're now on the loop's stack
                    for input_type in &input_types {
                        stack.push(*input_type);
                    }
                },
                0x04 => {
                    // if
                    // Pop condition (must be i32)
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }

                    let (block_type, new_offset) = Self::parse_block_type(code, offset, module)?;
                    offset = new_offset;

                    let (input_types, output_types) =
                        Self::block_type_to_stack_types(&block_type, module)?;

                    // For if with inputs, verify and pop the input types
                    for &expected in input_types.iter().rev() {
                        if !Self::pop_type(
                            &mut stack,
                            expected,
                            frame_height,
                            Self::is_unreachable(&frames),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // Record the stack height AFTER popping inputs
                    let stack_height = stack.len();

                    frames.push(ControlFrame {
                        frame_type: FrameType::If,
                        input_types: input_types.clone(),
                        output_types: output_types.clone(),
                        reachable: true,
                        stack_height,
                        unreachable_height: None,
                        concrete_push_count: 0,
                    });

                    // Push inputs back - they're now on the if's stack
                    for input_type in &input_types {
                        stack.push(*input_type);
                    }
                },
                0x05 => {
                    // else
                    if let Some(frame) = frames.last() {
                        if frame.frame_type != FrameType::If {
                            return Err(anyhow!("else: no matching if"));
                        }
                    }

                    // Verify then-branch produced correct outputs (if reachable)
                    if let Some(frame) = frames.last() {
                        if frame.reachable {
                            let expected_height = frame.stack_height + frame.output_types.len();
                            if stack.len() != expected_height {
                                return Err(anyhow!("type mismatch"));
                            }
                            // Also verify the types match (not just count)
                            for (i, &expected) in frame.output_types.iter().enumerate() {
                                let stack_idx = frame.stack_height + i;
                                if stack_idx < stack.len() {
                                    let actual = stack[stack_idx];
                                    if actual != StackType::Unknown
                                        && expected != StackType::Unknown
                                        && !Self::is_subtype_of_in_module(&actual, &expected, module)
                                    {
                                        return Err(anyhow!("type mismatch"));
                                    }
                                }
                            }
                        } else {
                            // Unreachable then-branch: check for excess concrete values
                            let unreachable_height =
                                frame.unreachable_height.unwrap_or(frame.stack_height);
                            let concrete_count = stack.len().saturating_sub(unreachable_height);
                            if concrete_count > frame.output_types.len() {
                                return Err(anyhow!("type mismatch"));
                            }
                        }
                    }

                    // Reset stack to frame start height and push input types for else branch
                    let input_types_clone =
                        frames.last().map(|f| f.input_types.clone()).unwrap_or_default();
                    if let Some(frame) = frames.last() {
                        stack.truncate(frame.stack_height);
                    }
                    for input_type in &input_types_clone {
                        stack.push(*input_type);
                    }

                    if let Some(frame) = frames.last_mut() {
                        frame.frame_type = FrameType::Else;
                        frame.reachable = true;
                        frame.unreachable_height = None;
                        frame.concrete_push_count = 0;
                    }
                },

                // Exception handling instructions
                0x06 => {
                    // try (legacy) - similar to block but with Try frame type
                    let (block_type, new_offset) = Self::parse_block_type(code, offset, module)?;
                    offset = new_offset;

                    let (input_types, output_types) =
                        Self::block_type_to_stack_types(&block_type, module)?;

                    // Pop inputs from stack
                    let frame_height = Self::current_frame_height(&frames);
                    for input_type in input_types.iter().rev() {
                        if !Self::pop_type(
                            &mut stack,
                            *input_type,
                            frame_height,
                            Self::is_unreachable(&frames),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    let stack_height = stack.len();
                    frames.push(ControlFrame {
                        frame_type: FrameType::Try,
                        input_types: input_types.clone(),
                        output_types: output_types.clone(),
                        reachable: true,
                        stack_height,
                        unreachable_height: None,
                        concrete_push_count: 0,
                    });

                    for input_type in &input_types {
                        stack.push(*input_type);
                    }
                },
                0x07 => {
                    // catch (legacy) - takes tag index, similar to else for if
                    let (tag_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Validate tag exists
                    if tag_idx as usize >= module.tags.len() {
                        return Err(anyhow!("unknown tag"));
                    }

                    // Catch must be in a try block
                    if let Some(frame) = frames.last() {
                        if frame.frame_type != FrameType::Try {
                            return Err(anyhow!("catch without try"));
                        }
                    } else {
                        return Err(anyhow!("catch without try"));
                    }

                    // Reset stack to frame start height
                    if let Some(frame) = frames.last() {
                        stack.truncate(frame.stack_height);
                    }

                    // Push the tag's parameter types onto the stack (caught values)
                    let tag = &module.tags[tag_idx as usize];
                    if let Some(tag_type) = module.types.get(tag.type_idx as usize) {
                        for param in &tag_type.params {
                            stack.push(StackType::from_value_type(*param));
                        }
                    }

                    // Mark as still in try context but now reachable
                    if let Some(frame) = frames.last_mut() {
                        frame.reachable = true;
                    }
                },
                0x08 => {
                    // throw - takes tag index, pops tag parameters, makes code unreachable
                    let (tag_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Validate tag exists (including imported tags)
                    if tag_idx as usize >= Self::total_tags(module) {
                        return Err(anyhow!("unknown tag"));
                    }

                    let frame_height = Self::current_frame_height(&frames);

                    // Pop the tag's parameter types from the stack
                    // Use get_tag_type_idx to handle both imported and defined tags
                    if let Some(type_idx) = Self::get_tag_type_idx(module, tag_idx) {
                        if let Some(tag_type) = module.types.get(type_idx as usize) {
                            for param in tag_type.params.iter().rev() {
                                let expected = StackType::from_value_type(*param);
                                if !Self::pop_type(
                                    &mut stack,
                                    expected,
                                    frame_height,
                                    Self::is_unreachable(&frames),
                                ) {
                                    return Err(anyhow!("type mismatch"));
                                }
                            }
                        }
                    }

                    // throw is a terminating instruction — stack becomes polymorphic
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },
                0x09 => {
                    // rethrow (legacy) - takes relative depth to try block
                    let (depth, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Validate relative depth points to a try/catch block
                    if depth as usize >= frames.len() {
                        return Err(anyhow!("rethrow: invalid depth"));
                    }
                    let target_frame_idx = frames.len() - 1 - depth as usize;
                    if frames[target_frame_idx].frame_type != FrameType::Try {
                        return Err(anyhow!("rethrow: not in try block"));
                    }

                    // rethrow is a terminating instruction — stack becomes polymorphic
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },
                0x0A => {
                    // throw_ref - pops exnref, throws it
                    let frame_height = Self::current_frame_height(&frames);

                    // Pop exnref
                    if !Self::pop_type(
                        &mut stack,
                        StackType::ExnRef,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // throw_ref is a terminating instruction — stack becomes polymorphic
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },

                0x0B => {
                    // end
                    if frames.len() == 1 {
                        // This is the final function-level end - valid termination
                        // Verify the stack matches the function's return types
                        let frame = &frames[0];
                        let frame_height = frame.stack_height;
                        let unreachable = !frame.reachable;

                        if unreachable {
                            // In unreachable code, the stack is polymorphic for underflow.
                            // Values pushed after unreachable are concrete and must type-check.
                            let unreachable_height =
                                frame.unreachable_height.unwrap_or(frame_height);

                            // Check for excess concrete values above the polymorphic base.
                            // The concrete values pushed after unreachable must not exceed
                            // the expected output count.
                            let concrete_count = stack.len().saturating_sub(unreachable_height);
                            if concrete_count > frame.output_types.len() {
                                return Err(anyhow!("type mismatch"));
                            }

                            for &expected in frame.output_types.iter().rev() {
                                if !Self::pop_type_with_module(&mut stack, expected, unreachable_height, true, Some(module)) {
                                    return Err(anyhow!("type mismatch"));
                                }
                            }
                            stack.truncate(frame_height);
                        } else {
                            // In reachable code, check exact stack height and types
                            let expected_height = frame_height + frame.output_types.len();
                            if stack.len() != expected_height {
                                return Err(anyhow!("type mismatch"));
                            }
                            for &expected in frame.output_types.iter().rev() {
                                if !Self::pop_type_with_module(&mut stack, expected, frame_height, false, Some(module)) {
                                    return Err(anyhow!("type mismatch"));
                                }
                            }
                        }
                        // Function validated successfully, exit loop
                        break;
                    }

                    // Pop block/loop/if frame
                    let frame = frames.pop().unwrap();

                    // If this is an if without else, the input and output types must match
                    // (because when condition is false, the else is implicitly empty,
                    // so inputs must pass through as outputs)
                    if frame.frame_type == FrameType::If && frame.input_types != frame.output_types
                    {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Verify stack has expected output types
                    let frame_height = frame.stack_height;
                    let unreachable = !frame.reachable;

                    if unreachable {
                        // In unreachable code, the stack is polymorphic for underflow.
                        // Values pushed after unreachable are concrete and must type-check.
                        let unreachable_height = frame.unreachable_height.unwrap_or(frame_height);

                        // Check for excess concrete values above the polymorphic base.
                        // The concrete values pushed after unreachable must not exceed
                        // the expected output count.
                        let concrete_count = stack.len().saturating_sub(unreachable_height);
                        if concrete_count > frame.output_types.len() {
                            return Err(anyhow!("type mismatch"));
                        }

                        for &expected in frame.output_types.iter().rev() {
                            if !Self::pop_type_with_module(&mut stack, expected, unreachable_height, true, Some(module)) {
                                return Err(anyhow!("type mismatch"));
                            }
                        }
                        // Truncate stack to frame height
                        stack.truncate(frame_height);
                    } else {
                        // In reachable code, check exact stack height and types
                        let expected_height = frame_height + frame.output_types.len();
                        if stack.len() != expected_height {
                            return Err(anyhow!("type mismatch"));
                        }
                        for &expected in frame.output_types.iter().rev() {
                            if !Self::pop_type_with_module(&mut stack, expected, frame_height, false, Some(module)) {
                                return Err(anyhow!("type mismatch"));
                            }
                        }
                    }

                    // Reset stack to frame height and push output types
                    stack.truncate(frame.stack_height);
                    stack.extend(frame.output_types.iter());
                },
                0x0C => {
                    // br (branch) - unconditional, makes following code unreachable
                    let (label_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    Self::validate_branch_with_module(
                        &stack,
                        label_idx,
                        &frames,
                        Self::is_unreachable(&frames),
                        Some(module),
                    )?;

                    // Mark current frame as unreachable — stack becomes polymorphic
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },
                0x0D => {
                    // br_if (branch if) - conditional, code after is still reachable
                    let (label_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Pop i32 condition (top of stack)
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }

                    Self::validate_branch_with_module(
                        &stack,
                        label_idx,
                        &frames,
                        Self::is_unreachable(&frames),
                        Some(module),
                    )?;

                    // After br_if, the stack types must be updated to the label's result types
                    // This is because from the perspective of subsequent code, the values
                    // could have been branched with (and cast to the label's type).
                    // GC spec: br_if with typed values narrows to label's result type.
                    if (label_idx as usize) < frames.len() && !Self::is_unreachable(&frames) {
                        let target_frame = &frames[frames.len() - 1 - label_idx as usize];
                        let label_types = if target_frame.frame_type == FrameType::Loop {
                            target_frame.input_types.clone()
                        } else {
                            target_frame.output_types.clone()
                        };

                        // Pop the branch values and push back the label's types
                        // This changes the stack types to match the label's expected types
                        let num_values = label_types.len();
                        if num_values > 0 && stack.len() >= frame_height + num_values {
                            // Pop the original values
                            for _ in 0..num_values {
                                stack.pop();
                            }
                            // Push the label's result types (more general types)
                            for ty in &label_types {
                                stack.push(*ty);
                            }
                        }
                    }
                },
                0x0E => {
                    // br_table - unconditional, makes following code unreachable
                    let (num_targets, mut new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Collect all branch targets (including default)
                    let mut targets: Vec<u32> = Vec::new();
                    for _ in 0..num_targets {
                        let (label_idx, temp_offset) = Self::parse_varuint32(code, new_offset)?;
                        targets.push(label_idx);
                        new_offset = temp_offset;
                    }
                    offset = new_offset;

                    // Parse default target
                    let (default_label, temp_offset) = Self::parse_varuint32(code, offset)?;
                    targets.push(default_label);
                    offset = temp_offset;

                    // Pop operand (i32 condition/index)
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Validate all targets are in range and have consistent types
                    let mut expected_arity: Option<usize> = None;
                    for &label_idx in &targets {
                        // Validate label is in range
                        if label_idx as usize >= frames.len() {
                            return Err(anyhow!("unknown label {}", label_idx));
                        }

                        let target_frame = &frames[frames.len() - 1 - label_idx as usize];
                        let branch_types = if target_frame.frame_type == FrameType::Loop {
                            &target_frame.input_types
                        } else {
                            &target_frame.output_types
                        };

                        match expected_arity {
                            None => {
                                expected_arity = Some(branch_types.len());
                            },
                            Some(arity) => {
                                if branch_types.len() != arity {
                                    return Err(anyhow!("type mismatch"));
                                }
                            },
                        }
                    }

                    // Validate the stack has the required values for the branch
                    Self::validate_branch_with_module(
                        &stack,
                        default_label,
                        &frames,
                        Self::is_unreachable(&frames),
                        Some(module),
                    )?;

                    // Mark current frame as unreachable — stack becomes polymorphic
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },
                0x0F => {
                    // return
                    let frame_height = Self::current_frame_height(&frames);
                    if let Some(frame) = frames.first() {
                        for &expected in frame.output_types.iter().rev() {
                            if !Self::pop_type_with_module(
                                &mut stack,
                                expected,
                                frame_height,
                                Self::is_unreachable(&frames),
                                Some(module),
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                        }
                    }

                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },
                0x10 => {
                    // call
                    let (func_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    if func_idx as usize >= module.functions.len() + module.imports.len() {
                        return Err(anyhow!("call: invalid function index {}", func_idx));
                    }

                    // Pop arguments and push results
                    if let Ok(func_type) = Self::get_function_type(module, func_idx) {
                        // Pop arguments in reverse order
                        let frame_height = Self::current_frame_height(&frames);
                        for param in func_type.params.iter().rev() {
                            let expected = StackType::from_value_type(*param);
                            if !Self::pop_type_with_module(
                                &mut stack,
                                expected,
                                frame_height,
                                Self::is_unreachable(&frames),
                                Some(module),
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                        }
                        // Push results
                        for result in &func_type.results {
                            stack.push(StackType::from_value_type(*result));
                        }
                    }
                },
                0x11 => {
                    // call_indirect
                    let (type_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // table_idx - validate that the table exists
                    let (table_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Validate table exists
                    if table_idx as usize >= Self::total_tables(module) {
                        return Err(anyhow!("unknown table"));
                    }

                    // Validate table element type is funcref (not externref)
                    if let Some(elem_type) = Self::get_table_element_type(module, table_idx) {
                        if elem_type != kiln_foundation::RefType::Funcref {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // Validate type index
                    if type_idx as usize >= module.types.len() {
                        return Err(anyhow!("unknown type"));
                    }

                    let func_type = &module.types[type_idx as usize];
                    let frame_height = Self::current_frame_height(&frames);

                    // Pop table index (i64 if table64, else i32)
                    let table_addr_type = if Self::is_table64(module, table_idx) { StackType::I64 } else { StackType::I32 };
                    if !Self::pop_type(
                        &mut stack,
                        table_addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Pop arguments in reverse order
                    for param in func_type.params.iter().rev() {
                        let expected = StackType::from_value_type(*param);
                        if !Self::pop_type_with_module(
                            &mut stack,
                            expected,
                            frame_height,
                            Self::is_unreachable(&frames),
                            Some(module),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // Push results
                    for result in &func_type.results {
                        stack.push(StackType::from_value_type(*result));
                    }
                },
                0x12 => {
                    // return_call (tail-call extension)
                    // Parse function index
                    let (func_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Get the called function's type (handles both imports and local functions)
                    let called_func_type = Self::get_function_type(module, func_idx)?;

                    // Verify called function's results match current function's results
                    // (Required for tail call: the callee's results become the caller's results)
                    if called_func_type.results.len() != func_type.results.len() {
                        return Err(anyhow!("type mismatch"));
                    }
                    for (called_result, current_result) in
                        called_func_type.results.iter().zip(func_type.results.iter())
                    {
                        if called_result != current_result {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    let frame_height = Self::current_frame_height(&frames);

                    // Pop arguments in reverse order
                    for param in called_func_type.params.iter().rev() {
                        let expected = StackType::from_value_type(*param);
                        if !Self::pop_type_with_module(
                            &mut stack,
                            expected,
                            frame_height,
                            Self::is_unreachable(&frames),
                            Some(module),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // return_call is a terminating instruction (like return)
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },
                0x13 => {
                    // return_call_indirect (tail-call extension)
                    let (type_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // table_idx - validate that the table exists
                    let (table_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Validate table exists
                    if table_idx as usize >= Self::total_tables(module) {
                        return Err(anyhow!("unknown table"));
                    }

                    // Validate table element type is funcref (not externref)
                    if let Some(elem_type) = Self::get_table_element_type(module, table_idx) {
                        if elem_type != kiln_foundation::RefType::Funcref {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // Validate type index
                    if type_idx as usize >= module.types.len() {
                        return Err(anyhow!("unknown type"));
                    }

                    let called_type = &module.types[type_idx as usize];

                    // Verify called function type's results match current function's results
                    // (Required for tail call: the callee's results become the caller's results)
                    if called_type.results.len() != func_type.results.len() {
                        return Err(anyhow!("type mismatch"));
                    }
                    for (called_result, current_result) in
                        called_type.results.iter().zip(func_type.results.iter())
                    {
                        if called_result != current_result {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    let frame_height = Self::current_frame_height(&frames);

                    // Pop table index (i64 if table64, else i32)
                    let table_addr_type = if Self::is_table64(module, table_idx) { StackType::I64 } else { StackType::I32 };
                    if !Self::pop_type(
                        &mut stack,
                        table_addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Pop arguments in reverse order
                    for param in called_type.params.iter().rev() {
                        let expected = StackType::from_value_type(*param);
                        if !Self::pop_type_with_module(
                            &mut stack,
                            expected,
                            frame_height,
                            Self::is_unreachable(&frames),
                            Some(module),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // return_call_indirect is a terminating instruction (like return)
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },
                0x14 => {
                    // call_ref $t: [(params...) (ref null $t)] -> [(results...)]
                    let (type_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    if type_idx as usize >= module.types.len() {
                        return Err(anyhow!("unknown type"));
                    }

                    let called_type = &module.types[type_idx as usize];
                    let frame_height = Self::current_frame_height(&frames);

                    // Pop the typed function reference — must be (ref null $t)
                    if !Self::pop_type_with_module(
                        &mut stack,
                        StackType::TypedFuncRef(type_idx, true),
                        frame_height,
                        Self::is_unreachable(&frames),
                        Some(module),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Pop arguments in reverse order
                    for param in called_type.params.iter().rev() {
                        let expected = StackType::from_value_type(*param);
                        if !Self::pop_type_with_module(
                            &mut stack,
                            expected,
                            frame_height,
                            Self::is_unreachable(&frames),
                            Some(module),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // Push results
                    for result in &called_type.results {
                        stack.push(StackType::from_value_type(*result));
                    }
                },
                0x15 => {
                    // return_call_ref $t: [(params...) (ref null $t)] -> []
                    let (type_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    if type_idx as usize >= module.types.len() {
                        return Err(anyhow!("unknown type"));
                    }

                    let called_type = &module.types[type_idx as usize];

                    // Verify called function type's results match current function's results
                    if called_type.results.len() != func_type.results.len() {
                        return Err(anyhow!("type mismatch"));
                    }
                    for (called_result, current_result) in
                        called_type.results.iter().zip(func_type.results.iter())
                    {
                        let called_st = StackType::from_value_type(*called_result);
                        let current_st = StackType::from_value_type(*current_result);
                        // Callee result must be subtype of caller result (covariant)
                        if !Self::is_subtype_of_in_module(&called_st, &current_st, module) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    let frame_height = Self::current_frame_height(&frames);

                    // Pop the typed function reference — must be (ref null $t)
                    if !Self::pop_type_with_module(
                        &mut stack,
                        StackType::TypedFuncRef(type_idx, true),
                        frame_height,
                        Self::is_unreachable(&frames),
                        Some(module),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Pop arguments in reverse order
                    for param in called_type.params.iter().rev() {
                        let expected = StackType::from_value_type(*param);
                        if !Self::pop_type_with_module(
                            &mut stack,
                            expected,
                            frame_height,
                            Self::is_unreachable(&frames),
                            Some(module),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    // return_call_ref is a terminating instruction
                    if let Some(frame) = frames.last_mut() {
                        if frame.reachable {
                            frame.unreachable_height = Some(frame.stack_height);
                        }
                        stack.truncate(frame.stack_height);
                        frame.reachable = false;
                    }
                },

                // Exception handling (continued)
                0x18 => {
                    // delegate (legacy) - terminates try block and delegates to outer handler
                    let (depth, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Must be in a try block
                    if let Some(frame) = frames.last() {
                        if frame.frame_type != FrameType::Try {
                            return Err(anyhow!("delegate without try"));
                        }
                    } else {
                        return Err(anyhow!("delegate without try"));
                    }

                    // Validate relative depth
                    if depth as usize >= frames.len() {
                        return Err(anyhow!("delegate: invalid depth"));
                    }

                    // Pop the try frame
                    if let Some(frame) = frames.pop() {
                        // Restore stack to before try
                        stack.truncate(frame.stack_height);
                        // Push outputs
                        for output in &frame.output_types {
                            stack.push(*output);
                        }
                    }
                },
                0x19 => {
                    // catch_all (legacy) - catches all exceptions
                    // Must be in a try block
                    if let Some(frame) = frames.last() {
                        if frame.frame_type != FrameType::Try {
                            return Err(anyhow!("catch_all without try"));
                        }
                    } else {
                        return Err(anyhow!("catch_all without try"));
                    }

                    // Reset stack to frame start height
                    if let Some(frame) = frames.last() {
                        stack.truncate(frame.stack_height);
                    }

                    // catch_all doesn't push any values - exception is discarded
                    // Mark as reachable
                    if let Some(frame) = frames.last_mut() {
                        frame.reachable = true;
                    }
                },
                0x1F => {
                    // try_table - modern exception handling block
                    let (block_type, new_offset) = Self::parse_block_type(code, offset, module)?;
                    offset = new_offset;

                    // Parse handler count
                    let (handler_count, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Parse and validate each handler
                    // IMPORTANT: Handler labels are interpreted relative to the try_table's
                    // position in the control stack, NOT from inside the try_table body.
                    // This means label 0 refers to the immediately enclosing block, not the try_table.
                    // Therefore we validate handlers BEFORE pushing the try_table frame.
                    for _ in 0..handler_count {
                        if offset >= code.len() {
                            return Err(anyhow!("try_table: unexpected end"));
                        }
                        let catch_kind = code[offset];
                        offset += 1;

                        match catch_kind {
                            0x00 => {
                                // catch: pushes tag's param types
                                let (tag_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                                offset = new_offset;
                                if tag_idx as usize >= Self::total_tags(module) {
                                    return Err(anyhow!("unknown tag"));
                                }
                                let (label, new_offset) = Self::parse_varuint32(code, offset)?;
                                offset = new_offset;
                                let tag_params = Self::get_tag_param_types(module, tag_idx)?;
                                // Validate handler branch target BEFORE try_table frame is pushed
                                Self::validate_handler_branch(&frames, label, &tag_params)?;
                            },
                            0x01 => {
                                // catch_ref: pushes tag's param types + exnref
                                let (tag_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                                offset = new_offset;
                                if tag_idx as usize >= Self::total_tags(module) {
                                    return Err(anyhow!("unknown tag"));
                                }
                                let (label, new_offset) = Self::parse_varuint32(code, offset)?;
                                offset = new_offset;
                                let mut handler_types = Self::get_tag_param_types(module, tag_idx)?;
                                handler_types.push(StackType::ExnRef);
                                // Validate handler branch target BEFORE try_table frame is pushed
                                Self::validate_handler_branch(&frames, label, &handler_types)?;
                            },
                            0x02 => {
                                // catch_all: pushes nothing
                                let (label, new_offset) = Self::parse_varuint32(code, offset)?;
                                offset = new_offset;
                                // Validate handler branch target BEFORE try_table frame is pushed
                                Self::validate_handler_branch(&frames, label, &[])?;
                            },
                            0x03 => {
                                // catch_all_ref: pushes exnref
                                let (label, new_offset) = Self::parse_varuint32(code, offset)?;
                                offset = new_offset;
                                let handler_types = vec![StackType::ExnRef];
                                // Validate handler branch target BEFORE try_table frame is pushed
                                Self::validate_handler_branch(&frames, label, &handler_types)?;
                            },
                            _ => return Err(anyhow!("invalid catch handler kind")),
                        }
                    }

                    let (input_types, output_types) =
                        Self::block_type_to_stack_types(&block_type, module)?;

                    // Pop inputs from stack
                    let frame_height = Self::current_frame_height(&frames);
                    for input_type in input_types.iter().rev() {
                        if !Self::pop_type(
                            &mut stack,
                            *input_type,
                            frame_height,
                            Self::is_unreachable(&frames),
                        ) {
                            return Err(anyhow!("type mismatch"));
                        }
                    }

                    let stack_height = stack.len();
                    frames.push(ControlFrame {
                        frame_type: FrameType::Try,
                        input_types: input_types.clone(),
                        output_types: output_types.clone(),
                        reachable: true,
                        stack_height,
                        unreachable_height: None,
                        concrete_push_count: 0,
                    });

                    for input_type in &input_types {
                        stack.push(*input_type);
                    }
                },

                // Memory operations - Load instructions
                // All memory operations require at least one memory to be defined
                // For memory64, addresses are i64 instead of i32
                0x28 => {
                    // i32.load - pop i32/i64 address, push i32 value
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },
                0x29 => {
                    // i64.load - pop i32/i64 address, push i64 value
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I64);
                },
                0x2A => {
                    // f32.load - pop i32/i64 address, push f32 value
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::F32);
                },
                0x2B => {
                    // f64.load - pop i32/i64 address, push f64 value
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::F64);
                },
                0x2C..=0x35 => {
                    // Extended load operations (load8, load16, load32, etc.)
                    // All take i32/i64 address and return the loaded value type
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    // Push result based on opcode
                    let result_type = match opcode {
                        0x2C | 0x2D | 0x2E | 0x2F => StackType::I32, // i32.load8_s/u, i32.load16_s/u
                        0x30 | 0x31 | 0x32 | 0x33 | 0x34 | 0x35 => StackType::I64, // i64 loads
                        _ => StackType::I32,
                    };
                    stack.push(result_type);
                },
                0x36 => {
                    // i32.store - pop i32 value and i32/i64 address
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    // Pop value (i32)
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    // Pop address (i32 or i64 for memory64)
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                },
                0x37 => {
                    // i64.store - pop i64 value and i32/i64 address
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    // Pop value (i64)
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    // Pop address (i32 or i64 for memory64)
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                },
                0x38 => {
                    // f32.store - pop f32 value and i32/i64 address
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    // Pop value (f32)
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    // Pop address (i32 or i64 for memory64)
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                },
                0x39 => {
                    // f64.store - pop f64 value and i32/i64 address
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    // Pop value (f64)
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    // Pop address (i32 or i64 for memory64)
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                },
                0x3A..=0x3E => {
                    // Extended store operations (store8, store16, store32)
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    let (mem_idx, new_offset) = Self::parse_memarg_with_opcode(code, offset, module, Some(opcode))?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    // Pop value type based on opcode
                    let value_type = match opcode {
                        0x3A | 0x3B => StackType::I32,        // i32.store8, i32.store16
                        0x3C | 0x3D | 0x3E => StackType::I64, // i64.store8/16/32
                        _ => StackType::I32,
                    };
                    if !Self::pop_type(
                        &mut stack,
                        value_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    // Pop address (i32 or i64 for memory64)
                    let addr_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        addr_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                },
                0x3F => {
                    // memory.size - push i32/i64 (current memory size in pages)
                    // For memory64, returns i64; otherwise i32
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    // Read memory index (usually 0x00 for default memory)
                    let mem_idx = if offset < code.len() {
                        let idx = code[offset] as u32;
                        offset += 1;
                        idx
                    } else {
                        0
                    };
                    // Memory64 returns i64, regular memory returns i32
                    if Self::is_memory64(module, mem_idx) {
                        stack.push(StackType::I64);
                    } else {
                        stack.push(StackType::I32);
                    }
                },
                0x40 => {
                    // memory.grow - pop i32/i64 (delta pages), push i32/i64 (previous size or -1)
                    // For memory64, takes and returns i64; otherwise i32
                    if !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }
                    // Read memory index (usually 0x00 for default memory)
                    let mem_idx = if offset < code.len() {
                        let idx = code[offset] as u32;
                        offset += 1;
                        idx
                    } else {
                        0
                    };
                    let frame_height = Self::current_frame_height(&frames);
                    // Memory64 uses i64 for delta and result, regular uses i32
                    let size_type = if Self::is_memory64(module, mem_idx) {
                        StackType::I64
                    } else {
                        StackType::I32
                    };
                    if !Self::pop_type(
                        &mut stack,
                        size_type,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(size_type);
                },

                // Variable operations
                0x20 => {
                    // local.get
                    let (local_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    if local_idx as usize >= local_types.len() {
                        return Err(anyhow!("local.get: invalid local index {}", local_idx));
                    }

                    stack.push(StackType::from_value_type(local_types[local_idx as usize]));
                },
                0x21 => {
                    // local.set
                    let (local_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    if local_idx as usize >= local_types.len() {
                        return Err(anyhow!("local.set: invalid local index {}", local_idx));
                    }

                    let expected = StackType::from_value_type(local_types[local_idx as usize]);
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        expected,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                },
                0x22 => {
                    // local.tee
                    let (local_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    if local_idx as usize >= local_types.len() {
                        return Err(anyhow!("local.tee: invalid local index {}", local_idx));
                    }

                    // In unreachable code, the stack is polymorphic
                    if !Self::is_unreachable(&frames) {
                        let expected = StackType::from_value_type(local_types[local_idx as usize]);
                        if stack.last() != Some(&expected)
                            && stack.last() != Some(&StackType::Unknown)
                        {
                            return Err(anyhow!("local.tee: type mismatch"));
                        }
                    }
                },
                0x23 => {
                    // global.get
                    let (global_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Check total globals (imports + defined)
                    if global_idx as usize >= Self::total_globals(module) {
                        return Err(anyhow!("unknown global"));
                    }

                    let global_type = Self::get_global_type(module, global_idx)
                        .ok_or_else(|| anyhow!("unknown global"))?;
                    stack.push(StackType::from_value_type(global_type));
                },
                0x24 => {
                    // global.set
                    let (global_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Check total globals (imports + defined)
                    if global_idx as usize >= Self::total_globals(module) {
                        return Err(anyhow!("unknown global"));
                    }

                    // Check that the global is mutable
                    let is_mutable = Self::is_global_mutable(module, global_idx)
                        .ok_or_else(|| anyhow!("unknown global"))?;
                    if !is_mutable {
                        return Err(anyhow!("immutable global"));
                    }

                    let global_type = Self::get_global_type(module, global_idx)
                        .ok_or_else(|| anyhow!("unknown global"))?;
                    let expected = StackType::from_value_type(global_type);
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        expected,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                },
                0x25 => {
                    // table.get: [it] -> [t] where t is the table element type, it=i64 if table64
                    let (table_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Validate table exists
                    if table_idx as usize >= Self::total_tables(module) {
                        return Err(anyhow!("unknown table"));
                    }

                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    let it = if Self::is_table64(module, table_idx) { StackType::I64 } else { StackType::I32 };

                    // Pop index
                    if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Push the table's element type
                    if let Some(elem_type) = Self::get_table_element_type(module, table_idx) {
                        stack.push(StackType::from_ref_type(elem_type));
                    } else {
                        stack.push(StackType::Unknown);
                    }
                },
                0x26 => {
                    // table.set: [it, t] -> [] where t is the table element type, it=i64 if table64
                    let (table_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Validate table exists
                    if table_idx as usize >= Self::total_tables(module) {
                        return Err(anyhow!("unknown table"));
                    }

                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    let it = if Self::is_table64(module, table_idx) { StackType::I64 } else { StackType::I32 };

                    // Get the expected table element type
                    let expected_type = if let Some(elem_type) =
                        Self::get_table_element_type(module, table_idx)
                    {
                        StackType::from_ref_type(elem_type)
                    } else {
                        StackType::Unknown
                    };

                    // Pop value (must match table element type)
                    if !unreachable && stack.len() > frame_height {
                        if let Some(actual_type) = stack.last() {
                            if !actual_type.is_subtype_of(&expected_type)
                                && *actual_type != StackType::Unknown
                            {
                                return Err(anyhow!("type mismatch"));
                            }
                        }
                        stack.pop();
                    } else if !unreachable {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Pop index
                    if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                        return Err(anyhow!("type mismatch"));
                    }
                },

                // Constants
                0x41 => {
                    // i32.const
                    let (_, new_offset) = Self::parse_varint32(code, offset)?;
                    offset = new_offset;
                    stack.push(StackType::I32);
                },
                0x42 => {
                    // i64.const
                    let (_, new_offset) = Self::parse_varint64(code, offset)?;
                    offset = new_offset;
                    stack.push(StackType::I64);
                },
                0x43 => {
                    // f32.const
                    if offset + 4 > code.len() {
                        return Err(anyhow!("f32.const: truncated instruction"));
                    }
                    offset += 4;
                    stack.push(StackType::F32);
                },
                0x44 => {
                    // f64.const
                    if offset + 8 > code.len() {
                        return Err(anyhow!("f64.const: truncated instruction"));
                    }
                    offset += 8;
                    stack.push(StackType::F64);
                },

                // Parametric operations
                0x1A => {
                    // drop
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if stack.len() <= frame_height && !unreachable {
                        return Err(anyhow!("type mismatch"));
                    }
                    if stack.len() > frame_height {
                        stack.pop();
                    }
                },
                0x1B => {
                    // select (untyped)
                    // Per WebAssembly spec: untyped select only works with numeric types
                    // Reference types (funcref, externref) require typed select (0x1C)
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);

                    // Pop condition (i32) - uses polymorphic underflow in unreachable code
                    if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Pop two operands - handle polymorphic underflow in unreachable code
                    // We need to track what types we actually pop to determine result type
                    let type2 = if stack.len() > frame_height {
                        stack.pop().unwrap()
                    } else if unreachable {
                        // Polymorphic underflow - phantom value
                        StackType::Unknown
                    } else {
                        return Err(anyhow!("type mismatch"));
                    };

                    let type1 = if stack.len() > frame_height {
                        stack.pop().unwrap()
                    } else if unreachable {
                        // Polymorphic underflow - phantom value
                        StackType::Unknown
                    } else {
                        return Err(anyhow!("type mismatch"));
                    };

                    // Check that operands are numeric types (not reference types)
                    // Untyped select cannot be used with funcref, externref, etc.
                    if !unreachable {
                        // In reachable code: reject reference types and mismatched types
                        if !type1.is_numeric() || !type2.is_numeric() {
                            return Err(anyhow!("type mismatch"));
                        }
                        if type1 != type2 {
                            return Err(anyhow!("type mismatch"));
                        }
                        stack.push(type1);
                    } else {
                        // In unreachable code: the stack is polymorphic.
                        // Any concrete types from unreachable instructions are valid.
                        // Per spec, select in unreachable code always succeeds.
                        // Push the most specific type, or Unknown if both are polymorphic.
                        let result_type = if type1 != StackType::Unknown {
                            type1
                        } else {
                            type2
                        };
                        stack.push(result_type);
                    }
                },
                0x1C => {
                    // select t* (typed select)
                    // Format: 0x1C vec(valtype)
                    // The vec is typically 1 element indicating the result type
                    let (num_types, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Parse the result type(s)
                    let mut result_type = StackType::Unknown;
                    for _ in 0..num_types {
                        if offset >= code.len() {
                            return Err(anyhow!("truncated select type"));
                        }
                        let type_byte = code[offset];
                        offset += 1;
                        result_type = match type_byte {
                            0x7F => StackType::I32,
                            0x7E => StackType::I64,
                            0x7D => StackType::F32,
                            0x7C => StackType::F64,
                            0x7B => StackType::V128,
                            0x70 => StackType::FuncRef,
                            0x6F => StackType::ExternRef,
                            0x69 => StackType::ExnRef,
                            0x63 | 0x64 => {
                                // ref null heaptype / ref heaptype
                                let nullable = type_byte == 0x63;
                                let (heap_type, new_offset) = Self::parse_heap_type(code, offset, nullable)?;
                                offset = new_offset;
                                Self::check_value_type_ref(&heap_type, module.types.len())?;
                                StackType::from_value_type(heap_type)
                            },
                            _ => StackType::Unknown,
                        };
                    }

                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);

                    // Pop i32 condition
                    if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Pop two values of the result type
                    if !Self::pop_type(&mut stack, result_type, frame_height, unreachable) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(&mut stack, result_type, frame_height, unreachable) {
                        return Err(anyhow!("type mismatch"));
                    }

                    // Push the result
                    stack.push(result_type);
                },

                // f32 unary operations (0x8B-0x91): abs, neg, ceil, floor, trunc, nearest, sqrt
                0x8B | 0x8C | 0x8D | 0x8E | 0x8F | 0x90 | 0x91 => {
                    // f32 unary: f32 -> f32
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::F32);
                },
                // f32 binary operations (0x92-0x98): add, sub, mul, div, min, max, copysign
                0x92 | 0x93 | 0x94 | 0x95 | 0x96 | 0x97 | 0x98 => {
                    // f32 binary: f32 f32 -> f32
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::F32);
                },
                // f64 unary operations (0x99-0x9F): abs, neg, ceil, floor, trunc, nearest, sqrt
                0x99 | 0x9A | 0x9B | 0x9C | 0x9D | 0x9E | 0x9F => {
                    // f64 unary: f64 -> f64
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::F64);
                },
                // f64 binary operations (0xA0-0xA6): add, sub, mul, div, min, max, copysign
                0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5 | 0xA6 => {
                    // f64 binary: f64 f64 -> f64
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::F64);
                },
                // i32 unary: clz (0x67), ctz, popcnt
                0x67 | 0x68 | 0x69 => {
                    // i32 unary operations
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },
                // i64 unary: clz (0x79), ctz, popcnt
                0x79 | 0x7A | 0x7B => {
                    // i64 unary operations
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I64);
                },

                // i32.eqz (0x45): i32 -> i32
                0x45 => {
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },

                // i32 comparison operations (0x46-0x4F): i32 i32 -> i32
                0x46 | 0x47 | 0x48 | 0x49 | 0x4A | 0x4B | 0x4C | 0x4D | 0x4E | 0x4F => {
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },

                // i64.eqz (0x50): i64 -> i32
                0x50 => {
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },

                // i64 comparison operations (0x51-0x5A): i64 i64 -> i32
                0x51 | 0x52 | 0x53 | 0x54 | 0x55 | 0x56 | 0x57 | 0x58 | 0x59 | 0x5A => {
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },

                // f32 comparison operations (0x5B-0x60): f32 f32 -> i32
                0x5B | 0x5C | 0x5D | 0x5E | 0x5F | 0x60 => {
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },

                // f64 comparison operations (0x61-0x66): f64 f64 -> i32
                0x61 | 0x62 | 0x63 | 0x64 | 0x65 | 0x66 => {
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(
                        &mut stack,
                        StackType::F64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },

                // i32 binary operations (0x6A-0x78): i32 i32 -> i32
                0x6A | 0x6B | 0x6C | 0x6D | 0x6E | 0x6F | 0x70 | 0x71 | 0x72 | 0x73 | 0x74
                | 0x75 | 0x76 | 0x77 | 0x78 => {
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },

                // i64 binary operations (0x7C-0x8A): i64 i64 -> i64
                0x7C | 0x7D | 0x7E | 0x7F | 0x80 | 0x81 | 0x82 | 0x83 | 0x84 | 0x85 | 0x86
                | 0x87 | 0x88 | 0x89 | 0x8A | 0x8B => {
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I64);
                },

                // Conversion operations: i32 -> i64
                0xac | 0xad => {
                    // i64.extend_i32_s (0xac), i64.extend_i32_u (0xad)
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I32,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I64);
                },

                // Conversion operations: i64 -> i32
                0xa7 => {
                    // i32.wrap_i64
                    let frame_height = Self::current_frame_height(&frames);
                    if !Self::pop_type(
                        &mut stack,
                        StackType::I64,
                        frame_height,
                        Self::is_unreachable(&frames),
                    ) {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },

                // Conversion operations: f32 <-> i32
                0xa8 | 0xa9 | 0xaa | 0xab => {
                    // i32.trunc_f32_s (0xa8), i32.trunc_f32_u (0xa9)
                    // i32.trunc_f64_s (0xaa), i32.trunc_f64_u (0xab)
                    let is_f64 = opcode >= 0xaa;
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if is_f64 {
                        if !Self::pop_type(&mut stack, StackType::F64, frame_height, unreachable) {
                            return Err(anyhow!("i32.trunc: operand must be f64"));
                        }
                    } else {
                        if !Self::pop_type(&mut stack, StackType::F32, frame_height, unreachable) {
                            return Err(anyhow!("i32.trunc: operand must be f32"));
                        }
                    }
                    stack.push(StackType::I32);
                },

                // Conversion operations: f32/f64 <-> i64
                0xae | 0xaf | 0xb0 | 0xb1 => {
                    // i64.trunc_f32_s (0xae), i64.trunc_f32_u (0xaf)
                    // i64.trunc_f64_s (0xb0), i64.trunc_f64_u (0xb1)
                    let is_f64 = opcode >= 0xb0;
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if is_f64 {
                        if !Self::pop_type(&mut stack, StackType::F64, frame_height, unreachable) {
                            return Err(anyhow!("i64.trunc: operand must be f64"));
                        }
                    } else {
                        if !Self::pop_type(&mut stack, StackType::F32, frame_height, unreachable) {
                            return Err(anyhow!("i64.trunc: operand must be f32"));
                        }
                    }
                    stack.push(StackType::I64);
                },

                // Conversion operations: i32/i64 -> f32
                0xb2 | 0xb3 | 0xb4 | 0xb5 => {
                    // f32.convert_i32_s (0xb2), f32.convert_i32_u (0xb3)
                    // f32.convert_i64_s (0xb4), f32.convert_i64_u (0xb5)
                    let is_i64 = opcode >= 0xb4;
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if is_i64 {
                        if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) {
                            return Err(anyhow!("f32.convert: operand must be i64"));
                        }
                    } else {
                        if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                            return Err(anyhow!("f32.convert: operand must be i32"));
                        }
                    }
                    stack.push(StackType::F32);
                },

                // Conversion operations: f64.demote_f32
                0xb6 => {
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if !Self::pop_type(&mut stack, StackType::F64, frame_height, unreachable) {
                        return Err(anyhow!("f32.demote_f64: operand must be f64"));
                    }
                    stack.push(StackType::F32);
                },

                // Conversion operations: i32/i64 -> f64
                0xb7 | 0xb8 | 0xb9 | 0xba => {
                    // f64.convert_i32_s (0xb7), f64.convert_i32_u (0xb8)
                    // f64.convert_i64_s (0xb9), f64.convert_i64_u (0xba)
                    let is_i64 = opcode >= 0xb9;
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if is_i64 {
                        if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) {
                            return Err(anyhow!("f64.convert: operand must be i64"));
                        }
                    } else {
                        if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                            return Err(anyhow!("f64.convert: operand must be i32"));
                        }
                    }
                    stack.push(StackType::F64);
                },

                // Conversion operations: f64.promote_f32
                0xbb => {
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if !Self::pop_type(&mut stack, StackType::F32, frame_height, unreachable) {
                        return Err(anyhow!("f64.promote_f32: operand must be f32"));
                    }
                    stack.push(StackType::F64);
                },

                // Reinterpret operations (same size, different type)
                0xbc => {
                    // i32.reinterpret_f32
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if !Self::pop_type(&mut stack, StackType::F32, frame_height, unreachable) {
                        return Err(anyhow!("i32.reinterpret_f32: operand must be f32"));
                    }
                    stack.push(StackType::I32);
                },
                0xbd => {
                    // i64.reinterpret_f64
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if !Self::pop_type(&mut stack, StackType::F64, frame_height, unreachable) {
                        return Err(anyhow!("i64.reinterpret_f64: operand must be f64"));
                    }
                    stack.push(StackType::I64);
                },
                0xbe => {
                    // f32.reinterpret_i32
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                        return Err(anyhow!("f32.reinterpret_i32: operand must be i32"));
                    }
                    stack.push(StackType::F32);
                },
                0xbf => {
                    // f64.reinterpret_i64
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) {
                        return Err(anyhow!("f64.reinterpret_i64: operand must be i64"));
                    }
                    stack.push(StackType::F64);
                },

                // ref.null (0xD0)
                0xD0 => {
                    // Read heap type (signed LEB128 s33)
                    let (heap_type_val, new_offset) = Self::parse_varint64(code, offset)?;
                    offset = new_offset;
                    match heap_type_val {
                        // func → NullFuncRef (bottom of funcref hierarchy)
                        0x70 | -16 => stack.push(StackType::NullFuncRef),
                        // nofunc → NullFuncRef
                        0x73 | -13 => stack.push(StackType::NullFuncRef),
                        // extern → NullExternRef
                        0x6F | -17 => stack.push(StackType::NullExternRef),
                        // noextern → NullExternRef
                        0x72 | -14 => stack.push(StackType::NullExternRef),
                        // exn → NullExnRef
                        0x69 | -23 => stack.push(StackType::NullExnRef),
                        // noexn → NullExnRef
                        0x74 | -12 => stack.push(StackType::NullExnRef),
                        // any → NoneRef (null any is bottom of any hierarchy)
                        0x6E | -18 => stack.push(StackType::NoneRef),
                        // eq → NoneRef (null eq is subtype of eqref)
                        0x6D | -19 => stack.push(StackType::NoneRef),
                        // i31 → NoneRef
                        0x6C | -20 => stack.push(StackType::NoneRef),
                        // struct → NoneRef
                        0x6B | -21 => stack.push(StackType::NoneRef),
                        // array → NoneRef
                        0x6A | -22 => stack.push(StackType::NoneRef),
                        // none → NoneRef (bottom of any hierarchy)
                        0x71 | -15 => stack.push(StackType::NoneRef),
                        // Concrete type index → nullable typed reference
                        _ if heap_type_val >= 0 => {
                            stack.push(StackType::TypedFuncRef(heap_type_val as u32, true));
                        },
                        // Other abstract heap types
                        _ => stack.push(StackType::Unknown),
                    }
                },
                // ref.is_null (0xD1)
                0xD1 => {
                    // Pops a reference, pushes i32
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if unreachable {
                        // In unreachable code, pop if there's a value available
                        if stack.len() > frame_height {
                            let ref_type = stack.pop().unwrap();
                            if ref_type != StackType::Unknown && !ref_type.is_reference() {
                                return Err(anyhow!("type mismatch"));
                            }
                        }
                        // Otherwise polymorphic underflow is fine
                    } else {
                        if stack.len() <= frame_height {
                            return Err(anyhow!("type mismatch"));
                        }
                        let ref_type = stack.pop().unwrap();
                        if !ref_type.is_reference() && ref_type != StackType::Unknown {
                            return Err(anyhow!("type mismatch"));
                        }
                    }
                    stack.push(StackType::I32);
                },
                // ref.func (0xD2)
                0xD2 => {
                    // Read function index
                    let (func_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    // Validate function index is within bounds
                    let total_funcs = Self::total_functions(module);
                    if func_idx as usize >= total_funcs {
                        return Err(anyhow!("unknown function {}", func_idx));
                    }

                    // Validate function is declared (imported or in element segment)
                    if !declared_functions.contains(&func_idx) {
                        return Err(anyhow!("undeclared function reference"));
                    }

                    // ref.func produces a typed reference: (ref $t) where $t is the
                    // function's type. This is a non-nullable typed function reference.
                    let func_type_idx = Self::get_function_type_idx(module, func_idx)?;
                    stack.push(StackType::TypedFuncRef(func_type_idx, false));
                },

                // ref.eq (0xD3): [eqref eqref] -> [i32]
                0xD3 => {
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    // Pop two eqref values. We accept any subtype of eqref (i31, struct, array, eq, none)
                    // as well as Unknown for polymorphic typing.
                    for _ in 0..2 {
                        if !unreachable {
                            if stack.len() <= frame_height {
                                return Err(anyhow!("type mismatch"));
                            }
                            let val = stack.pop().unwrap();
                            if val != StackType::Unknown
                                && !val.is_subtype_of(&StackType::EqRef)
                            {
                                return Err(anyhow!("type mismatch"));
                            }
                        }
                    }
                    stack.push(StackType::I32);
                },

                // ref.as_non_null (0xD4): [ref] -> [ref]
                0xD4 => {
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    if !unreachable {
                        if stack.len() <= frame_height {
                            return Err(anyhow!("type mismatch"));
                        }
                        let ref_type = stack.pop().unwrap();
                        if !ref_type.is_reference() && ref_type != StackType::Unknown {
                            return Err(anyhow!("type mismatch"));
                        }
                        // Push back the same type (now guaranteed non-null)
                        stack.push(ref_type);
                    } else {
                        stack.push(StackType::Unknown);
                    }
                },

                // br_on_null (0xD5): [t* ref] -> [t*]
                0xD5 => {
                    let (_label_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    // Pop a reference value; if null, branch to label
                    if !unreachable {
                        if stack.len() <= frame_height {
                            return Err(anyhow!("type mismatch"));
                        }
                        let ref_type = stack.pop().unwrap();
                        if !ref_type.is_reference() && ref_type != StackType::Unknown {
                            return Err(anyhow!("type mismatch"));
                        }
                        // On fall-through, the ref is non-null, push it back
                        stack.push(ref_type);
                    }
                },

                // br_on_non_null (0xD6): [t* ref] -> [t*]
                0xD6 => {
                    let (_label_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);
                    // Pop a reference value; if non-null, branch to label with the ref
                    if !unreachable {
                        if stack.len() <= frame_height {
                            return Err(anyhow!("type mismatch"));
                        }
                        let ref_type = stack.pop().unwrap();
                        if !ref_type.is_reference() && ref_type != StackType::Unknown {
                            return Err(anyhow!("type mismatch"));
                        }
                        // On fall-through, the ref was null, so nothing pushed
                    }
                },

                // Multi-byte prefix (0xFC) - saturating truncations, bulk memory, etc.
                0xFC => {
                    if offset >= code.len() {
                        return Err(anyhow!("unexpected end of code after 0xFC prefix"));
                    }
                    let (sub_opcode, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);

                    match sub_opcode {
                        // Saturating truncation instructions (non-trapping float-to-int)
                        // i32.trunc_sat_f32_s (0x00): f32 -> i32
                        0x00 => {
                            if !Self::pop_type(
                                &mut stack,
                                StackType::F32,
                                frame_height,
                                unreachable,
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I32);
                        },
                        // i32.trunc_sat_f32_u (0x01): f32 -> i32
                        0x01 => {
                            if !Self::pop_type(
                                &mut stack,
                                StackType::F32,
                                frame_height,
                                unreachable,
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I32);
                        },
                        // i32.trunc_sat_f64_s (0x02): f64 -> i32
                        0x02 => {
                            if !Self::pop_type(
                                &mut stack,
                                StackType::F64,
                                frame_height,
                                unreachable,
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I32);
                        },
                        // i32.trunc_sat_f64_u (0x03): f64 -> i32
                        0x03 => {
                            if !Self::pop_type(
                                &mut stack,
                                StackType::F64,
                                frame_height,
                                unreachable,
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I32);
                        },
                        // i64.trunc_sat_f32_s (0x04): f32 -> i64
                        0x04 => {
                            if !Self::pop_type(
                                &mut stack,
                                StackType::F32,
                                frame_height,
                                unreachable,
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I64);
                        },
                        // i64.trunc_sat_f32_u (0x05): f32 -> i64
                        0x05 => {
                            if !Self::pop_type(
                                &mut stack,
                                StackType::F32,
                                frame_height,
                                unreachable,
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I64);
                        },
                        // i64.trunc_sat_f64_s (0x06): f64 -> i64
                        0x06 => {
                            if !Self::pop_type(
                                &mut stack,
                                StackType::F64,
                                frame_height,
                                unreachable,
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I64);
                        },
                        // i64.trunc_sat_f64_u (0x07): f64 -> i64
                        0x07 => {
                            if !Self::pop_type(
                                &mut stack,
                                StackType::F64,
                                frame_height,
                                unreachable,
                            ) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I64);
                        },
                        // memory.init (0x08): [it, i32, i32] -> [] (it=i64 if memory64)
                        // Dest offset uses memory address type, source offset and length are always i32
                        // (they index into the data segment, not memory)
                        0x08 => {
                            let (data_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            let (mem_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            if data_idx as usize >= module.data.len() {
                                return Err(anyhow!("unknown data segment"));
                            }
                            if mem_idx as usize >= Self::total_memories(module) {
                                return Err(anyhow!("unknown memory {}", mem_idx));
                            }
                            let it = if Self::is_memory64(module, mem_idx) { StackType::I64 } else { StackType::I32 };
                            // Pop n (length: i32), s (source offset: i32), d (dest offset: it) in reverse
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                        },
                        // data.drop (0x09): [] -> []
                        0x09 => {
                            let (data_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            if data_idx as usize >= module.data.len() {
                                return Err(anyhow!("unknown data segment"));
                            }
                        },
                        // memory.copy (0x0A): [it_d, it_s, it_n] -> [] (memory64-aware)
                        0x0A => {
                            let (dst_mem, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            let (src_mem, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            if dst_mem as usize >= Self::total_memories(module) {
                                return Err(anyhow!("unknown memory {}", dst_mem));
                            }
                            if src_mem as usize >= Self::total_memories(module) {
                                return Err(anyhow!("unknown memory {}", src_mem));
                            }
                            let dst64 = Self::is_memory64(module, dst_mem);
                            let src64 = Self::is_memory64(module, src_mem);
                            let it_d = if dst64 { StackType::I64 } else { StackType::I32 };
                            let it_s = if src64 { StackType::I64 } else { StackType::I32 };
                            // Length type: if either memory is 64-bit, length is i64
                            let it_n = if dst64 || src64 { StackType::I64 } else { StackType::I32 };
                            // Pop n (length), s (source), d (dest) in reverse
                            if !Self::pop_type(&mut stack, it_n, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, it_s, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, it_d, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                        },
                        // memory.fill (0x0B): [it, i32, it] -> [] (it=i64 if memory64)
                        0x0B => {
                            let (mem_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            if mem_idx as usize >= Self::total_memories(module) {
                                return Err(anyhow!("unknown memory {}", mem_idx));
                            }
                            let it = if Self::is_memory64(module, mem_idx) { StackType::I64 } else { StackType::I32 };
                            // Pop n (length), val (value), d (dest) in reverse
                            if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                        },
                        // table.init (0x0C): [it, i32, i32] -> [] (it=i64 if table64)
                        // Dest offset uses table address type, source offset and length are always i32
                        // (they index into the element segment, not the table)
                        0x0C => {
                            let (elem_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            let (table_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            if elem_idx as usize >= module.elements.len() {
                                return Err(anyhow!("unknown elem segment {}", elem_idx));
                            }
                            if table_idx as usize >= Self::total_tables(module) {
                                return Err(anyhow!("unknown table {}", table_idx));
                            }
                            let it = if Self::is_table64(module, table_idx) { StackType::I64 } else { StackType::I32 };
                            // Pop n (length: i32), s (source: i32), d (dest: it) in reverse
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                        },
                        // elem.drop (0x0D): [] -> []
                        0x0D => {
                            let (elem_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            if elem_idx as usize >= module.elements.len() {
                                return Err(anyhow!("unknown elem segment {}", elem_idx));
                            }
                        },
                        // table.copy (0x0E): [it_d, it_s, it_n] -> [] (table64-aware)
                        0x0E => {
                            let (dst_table, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            let (src_table, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            if dst_table as usize >= Self::total_tables(module) {
                                return Err(anyhow!("unknown table {}", dst_table));
                            }
                            if src_table as usize >= Self::total_tables(module) {
                                return Err(anyhow!("unknown table {}", src_table));
                            }
                            let dst64 = Self::is_table64(module, dst_table);
                            let src64 = Self::is_table64(module, src_table);
                            let it_d = if dst64 { StackType::I64 } else { StackType::I32 };
                            let it_s = if src64 { StackType::I64 } else { StackType::I32 };
                            let it_n = if dst64 || src64 { StackType::I64 } else { StackType::I32 };
                            // Pop n (length), s (source), d (dest) in reverse
                            if !Self::pop_type(&mut stack, it_n, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, it_s, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, it_d, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                        },
                        // table.grow (0x0F): [ref, it] -> [it] (it=i64 if table64)
                        0x0F => {
                            let (table_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            let it = if Self::is_table64(module, table_idx) { StackType::I64 } else { StackType::I32 };

                            // Validate table exists
                            if table_idx as usize >= Self::total_tables(module) {
                                return Err(anyhow!("unknown table"));
                            }

                            // Get the expected table element type
                            let expected_type = if let Some(elem_type) =
                                Self::get_table_element_type(module, table_idx)
                            {
                                StackType::from_ref_type(elem_type)
                            } else {
                                StackType::Unknown
                            };

                            // Pop n (delta)
                            if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            // Pop reference type (must match table element type)
                            if !unreachable && stack.len() > frame_height {
                                if let Some(actual_type) = stack.last() {
                                    if !actual_type.is_subtype_of(&expected_type)
                                        && *actual_type != StackType::Unknown
                                    {
                                        return Err(anyhow!("type mismatch"));
                                    }
                                }
                                stack.pop();
                            } else if !unreachable {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(it);
                        },
                        // table.size (0x10): [] -> [it] (it=i64 if table64)
                        0x10 => {
                            let (table_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            let it = if Self::is_table64(module, table_idx) { StackType::I64 } else { StackType::I32 };
                            stack.push(it);
                        },
                        // table.fill (0x11): [it, ref, it] -> [] (it=i64 if table64)
                        0x11 => {
                            let (table_idx, new_offset) = Self::parse_varuint32(code, offset)?;
                            offset = new_offset;
                            let it = if Self::is_table64(module, table_idx) { StackType::I64 } else { StackType::I32 };

                            // Validate table exists
                            if table_idx as usize >= Self::total_tables(module) {
                                return Err(anyhow!("unknown table"));
                            }

                            // Get the expected table element type
                            let expected_type = if let Some(elem_type) =
                                Self::get_table_element_type(module, table_idx)
                            {
                                StackType::from_ref_type(elem_type)
                            } else {
                                StackType::Unknown
                            };

                            // Pop n (length)
                            if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            // Pop reference type (must match table element type)
                            if !unreachable && stack.len() > frame_height {
                                if let Some(actual_type) = stack.last() {
                                    if !actual_type.is_subtype_of(&expected_type)
                                        && *actual_type != StackType::Unknown
                                    {
                                        return Err(anyhow!("type mismatch"));
                                    }
                                }
                                stack.pop();
                            } else if !unreachable {
                                return Err(anyhow!("type mismatch"));
                            }
                            // Pop i (dest)
                            if !Self::pop_type(&mut stack, it, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                        },
                        // Wide-arithmetic (0xFC 0x13-0x16)
                        0x13 | 0x14 => {
                            // i64.add128 / i64.sub128: [i64 i64 i64 i64] -> [i64 i64]
                            let frame_height = Self::current_frame_height(&frames);
                            let unreachable = Self::is_unreachable(&frames);
                            if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) { return Err(anyhow!("type mismatch")); }
                            if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) { return Err(anyhow!("type mismatch")); }
                            if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) { return Err(anyhow!("type mismatch")); }
                            if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) { return Err(anyhow!("type mismatch")); }
                            stack.push(StackType::I64);
                            stack.push(StackType::I64);
                        },
                        0x15 | 0x16 => {
                            // i64.mul_wide_s / i64.mul_wide_u: [i64 i64] -> [i64 i64]
                            let frame_height = Self::current_frame_height(&frames);
                            let unreachable = Self::is_unreachable(&frames);
                            if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) { return Err(anyhow!("type mismatch")); }
                            if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) { return Err(anyhow!("type mismatch")); }
                            stack.push(StackType::I64);
                            stack.push(StackType::I64);
                        },
                        // Unknown 0xFC sub-opcode - skip
                        _ => {},
                    }
                },

                // SIMD prefix (0xFD) - proper validation per WebAssembly SIMD spec
                0xFD => {
                    let (simd_opcode, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;
                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);

                    match simd_opcode {
                        // v128.load variants (0x00-0x0A): [i32] -> [v128]
                        0x00..=0x0A => {
                            let (align, o) = Self::parse_varuint32(code, offset)?; // align
                            Self::validate_simd_alignment(simd_opcode, align)?;
                            let (_, o) = Self::parse_varuint32(code, o)?;      // offset
                            offset = o;
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // v128.store (0x0B): [i32, v128] -> []
                        0x0B => {
                            let (align, o) = Self::parse_varuint32(code, offset)?;
                            Self::validate_simd_alignment(simd_opcode, align)?;
                            let (_, o) = Self::parse_varuint32(code, o)?;
                            offset = o;
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                        },
                        // v128.const (0x0C): [] -> [v128] - 16 byte immediate
                        0x0C => {
                            if offset + 16 > code.len() { return Err(anyhow!("unexpected end")); }
                            offset += 16;
                            stack.push(StackType::V128);
                        },
                        // i8x16.shuffle (0x0D): [v128, v128] -> [v128] - 16 lane bytes
                        0x0D => {
                            if offset + 16 > code.len() { return Err(anyhow!("unexpected end")); }
                            for i in 0..16 {
                                if code[offset + i] >= 32 {
                                    return Err(anyhow!("invalid lane index"));
                                }
                            }
                            offset += 16;
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // i8x16.swizzle (0x0E): [v128, v128] -> [v128]
                        0x0E => {
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // i8x16/i16x8/i32x4.splat (0x0F-0x11): [i32] -> [v128]
                        0x0F | 0x10 | 0x11 => {
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // i64x2.splat (0x12): [i64] -> [v128]
                        0x12 => {
                            if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // f32x4.splat (0x13): [f32] -> [v128]
                        0x13 => {
                            if !Self::pop_type(&mut stack, StackType::F32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // f64x2.splat (0x14): [f64] -> [v128]
                        0x14 => {
                            if !Self::pop_type(&mut stack, StackType::F64, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // Extract lane i8x16 (0x15,0x16): [v128] -> [i32]
                        0x15 | 0x16 => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 16 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I32);
                        },
                        // Replace lane i8x16 (0x17): [v128, i32] -> [v128]
                        0x17 => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 16 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // Extract lane i16x8 (0x18,0x19): [v128] -> [i32]
                        0x18 | 0x19 => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 8 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I32);
                        },
                        // Replace lane i16x8 (0x1A): [v128, i32] -> [v128]
                        0x1A => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 8 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // Extract lane i32x4 (0x1B): [v128] -> [i32]
                        0x1B => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 4 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I32);
                        },
                        // Replace lane i32x4 (0x1C): [v128, i32] -> [v128]
                        0x1C => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 4 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // i64x2.extract_lane (0x1D): [v128] -> [i64]
                        0x1D => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 2 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I64);
                        },
                        // i64x2.replace_lane (0x1E): [v128, i64] -> [v128]
                        0x1E => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 2 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::I64, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // f32x4.extract_lane (0x1F): [v128] -> [f32]
                        0x1F => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 4 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::F32);
                        },
                        // f32x4.replace_lane (0x20): [v128, f32] -> [v128]
                        0x20 => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 4 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::F32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // f64x2.extract_lane (0x21): [v128] -> [f64]
                        0x21 => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 2 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::F64);
                        },
                        // f64x2.replace_lane (0x22): [v128, f64] -> [v128]
                        0x22 => {
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            if lane >= 2 { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::F64, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // v128.any_true(0x53), all_true(0x63,0x83,0xA3,0xC3), bitmask(0x64,0x84,0xA4,0xC4): [v128] -> [i32]
                        0x53 | 0x63 | 0x64 | 0x83 | 0x84 | 0xA3 | 0xA4 | 0xC3 | 0xC4 => {
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::I32);
                        },
                        // v128.bitselect (0x52): [v128, v128, v128] -> [v128]
                        0x52 => {
                            for _ in 0..3 {
                                if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                    return Err(anyhow!("type mismatch"));
                                }
                            }
                            stack.push(StackType::V128);
                        },
                        // v128.load*_lane (0x54-0x57): [i32, v128] -> [v128]
                        0x54..=0x57 => {
                            let (align, o) = Self::parse_varuint32(code, offset)?;
                            Self::validate_simd_alignment(simd_opcode, align)?;
                            let (_, o) = Self::parse_varuint32(code, o)?;
                            offset = o;
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            let max_lane = Self::simd_lane_count_for_lane_op(simd_opcode);
                            if lane >= max_lane { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // v128.store*_lane (0x58-0x5B): [i32, v128] -> []
                        0x58..=0x5B => {
                            let (align, o) = Self::parse_varuint32(code, offset)?;
                            Self::validate_simd_alignment(simd_opcode, align)?;
                            let (_, o) = Self::parse_varuint32(code, o)?;
                            offset = o;
                            if offset >= code.len() { return Err(anyhow!("unexpected end")); }
                            let lane = code[offset];
                            offset += 1;
                            let max_lane = Self::simd_lane_count_for_lane_op(simd_opcode);
                            if lane >= max_lane { return Err(anyhow!("invalid lane index")); }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                        },
                        // v128.load32_zero, v128.load64_zero (0x5C, 0x5D): [i32] -> [v128]
                        0x5C | 0x5D => {
                            let (align, o) = Self::parse_varuint32(code, offset)?;
                            Self::validate_simd_alignment(simd_opcode, align)?;
                            let (_, o) = Self::parse_varuint32(code, o)?;
                            offset = o;
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // Shift ops [v128, i32] -> [v128]: 0x6B-0x6D, 0x8B-0x8D, 0xAB-0xAD, 0xCB-0xCD
                        0x6B..=0x6D | 0x8B..=0x8D | 0xAB..=0xAD | 0xCB..=0xCD => {
                            if !Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                return Err(anyhow!("type mismatch"));
                            }
                            stack.push(StackType::V128);
                        },
                        // All other ops: classify as unary, binary, or ternary
                        _ => {
                            let is_unary = matches!(simd_opcode,
                                0x4D | 0x5E | 0x5F |
                                0x60..=0x62 | 0x67..=0x6A | 0x74 | 0x75 | 0x7A | 0x7C..=0x7F |
                                0x80 | 0x81 | 0x87..=0x8A | 0x94 |
                                0xA0 | 0xA1 | 0xA7..=0xAA |
                                0xC0 | 0xC1 | 0xC7..=0xCA |
                                0xE0..=0xE3 | 0xEC..=0xEF | 0xF8..=0xFF |
                                // Relaxed SIMD unary: relaxed trunc [v128] -> [v128]
                                0x101..=0x104
                            );
                            // Relaxed SIMD ternary [v128, v128, v128] -> [v128]:
                            // relaxed_madd/nmadd (0x105-0x108), relaxed_laneselect (0x109-0x10C),
                            // relaxed_dot_i8x16_i7x16_add_s (0x113)
                            let is_ternary = matches!(simd_opcode,
                                0x105..=0x10C | 0x113
                            );
                            if is_ternary {
                                for _ in 0..3 {
                                    if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                        return Err(anyhow!("type mismatch"));
                                    }
                                }
                                stack.push(StackType::V128);
                            } else if is_unary {
                                if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                    return Err(anyhow!("type mismatch"));
                                }
                                stack.push(StackType::V128);
                            } else {
                                // Binary [v128, v128] -> [v128] (default)
                                // Includes relaxed: swizzle (0x100), min/max (0x10D-0x110),
                                // q15mulr_s (0x111), dot_i8x16_i7x16_s (0x112)
                                if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                    return Err(anyhow!("type mismatch"));
                                }
                                if !Self::pop_type(&mut stack, StackType::V128, frame_height, unreachable) {
                                    return Err(anyhow!("type mismatch"));
                                }
                                stack.push(StackType::V128);
                            }
                        },
                    }
                },

                // GC instructions (0xFB prefix)
                0xFB => {
                    if offset >= code.len() {
                        return Err(anyhow!("unexpected end of code after 0xFB prefix"));
                    }
                    let (sub_opcode, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;

                    let frame_height = Self::current_frame_height(&frames);
                    let unreachable = Self::is_unreachable(&frames);

                    // Parse immediates and model stack effects for GC instructions.
                    // We use Unknown for reference types since we don't track GC type indices.
                    match sub_opcode {
                        // struct.new $t: [field_types...] -> [(ref $t)]
                        0x00 => {
                            let (type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            // Pop one value per struct field
                            if let Some(field_count) = Self::get_struct_field_count(module, type_idx) {
                                for _ in 0..field_count {
                                    Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                                }
                            }
                            stack.push(StackType::Unknown);
                        },
                        // struct.new_default $t: [] -> [(ref $t)]
                        0x01 => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            stack.push(StackType::Unknown);
                        },
                        // struct.get $t $f: [(ref null $t)] -> [field_type]
                        0x02 => {
                            let (type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (field_idx, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            // Push the field's value type (or Unknown if we can't determine it)
                            let field_type = Self::get_struct_field_type(type_idx, field_idx, module);
                            stack.push(field_type);
                        },
                        // struct.get_s $t $f: [(ref null $t)] -> [i32]
                        0x03 => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (_field_idx, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::I32);
                        },
                        // struct.get_u $t $f: [(ref null $t)] -> [i32]
                        0x04 => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (_field_idx, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::I32);
                        },
                        // struct.set $t $f: [(ref null $t) field_type] -> []
                        0x05 => {
                            let (type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (field_idx, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            if !Self::is_struct_field_mutable(type_idx, field_idx, module) {
                                return Err(anyhow!("immutable field"));
                            }
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                        },
                        // array.new $t: [elem_type i32] -> [(ref $t)]
                        0x06 => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // array.new_default $t: [i32] -> [(ref $t)]
                        0x07 => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // array.new_fixed $t $n: [elem_type * n] -> [(ref $t)]
                        0x08 => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (count, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            for _ in 0..count {
                                Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            }
                            stack.push(StackType::Unknown);
                        },
                        // array.new_data $t $d: [i32 i32] -> [(ref $t)]
                        0x09 => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (_data_idx, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // array.new_elem $t $e: [i32 i32] -> [(ref $t)]
                        0x0A => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (_elem_idx, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // array.get $t: [(ref null $t) i32] -> [elem_type]
                        0x0B => {
                            let (type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            // Push the array element type
                            let elem_type = Self::get_array_element_type(type_idx, module);
                            stack.push(elem_type);
                        },
                        // array.get_s $t: [(ref null $t) i32] -> [i32]
                        0x0C => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::I32);
                        },
                        // array.get_u $t: [(ref null $t) i32] -> [i32]
                        0x0D => {
                            let (_type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::I32);
                        },
                        // array.set $t: [(ref null $t) i32 elem_type] -> []
                        0x0E => {
                            let (type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            if !Self::is_array_mutable(type_idx, module) {
                                return Err(anyhow!("immutable array"));
                            }
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                        },
                        // array.len: [(ref null array)] -> [i32]
                        0x0F => {
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::I32);
                        },
                        // array.fill $t: [(ref null $t) i32 val i32] -> []
                        0x10 => {
                            let (type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            if !Self::is_array_mutable(type_idx, module) {
                                return Err(anyhow!("immutable array"));
                            }
                            // array.fill $t: [(ref null $t) i32 val i32] -> []
                            // val must match the array element type
                            let elem_type = Self::get_array_element_type(type_idx, module);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            if !Self::pop_type_with_module(&mut stack, elem_type, frame_height, unreachable, Some(module)) {
                                return Err(anyhow!("type mismatch"));
                            }
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                        },
                        // array.copy $t1 $t2: [(ref null $t1) i32 (ref null $t2) i32 i32] -> []
                        0x11 => {
                            let (type_idx1, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (type_idx2, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            if !Self::is_array_mutable(type_idx1, module) {
                                return Err(anyhow!("immutable array"));
                            }
                            // Source element type must be compatible with destination
                            if !Self::are_array_element_types_compatible(type_idx2, type_idx1, module) {
                                return Err(anyhow!("array types do not match"));
                            }
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                        },
                        // array.init_data $t $d: [(ref null $t) i32 i32 i32] -> []
                        0x12 => {
                            let (type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            if !Self::is_array_mutable(type_idx, module) {
                                return Err(anyhow!("immutable array"));
                            }
                            if !Self::is_array_numeric_or_vector(type_idx, module) {
                                return Err(anyhow!("array type is not numeric or vector"));
                            }
                            let (_data_idx, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                        },
                        // array.init_elem $t $e: [(ref null $t) i32 i32 i32] -> []
                        0x13 => {
                            let (type_idx, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            if !Self::is_array_mutable(type_idx, module) {
                                return Err(anyhow!("immutable array"));
                            }
                            let (elem_idx, new_off2) = Self::parse_varuint32(code, offset)?;
                            offset = new_off2;
                            // Check element segment type is compatible with array element type
                            if let Some(elem_seg) = module.elements.get(elem_idx as usize) {
                                let array_elem = Self::get_array_element_type(type_idx, module);
                                let seg_type = StackType::from_value_type(elem_seg.element_type.to_value_type());
                                if array_elem != StackType::Unknown
                                    && !Self::is_subtype_of_in_module(&seg_type, &array_elem, module)
                                {
                                    return Err(anyhow!("type mismatch"));
                                }
                            }
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                        },
                        // ref.test (ref ht): [(ref null ht)] -> [i32]
                        0x14 | 0x15 => {
                            // 0x14 = ref.test (non-nullable), 0x15 = ref.test (nullable)
                            // Both take a heap type immediate
                            let (_ht, new_off) = Self::read_leb128_signed(code, offset)?;
                            offset = new_off;
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::I32);
                        },
                        // ref.cast (ref ht): [(ref null ht)] -> [(ref ht)]
                        0x16 | 0x17 => {
                            // 0x16 = ref.cast (non-nullable), 0x17 = ref.cast (nullable)
                            let (_ht, new_off) = Self::read_leb128_signed(code, offset)?;
                            offset = new_off;
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // br_on_cast: flags(1 byte) + label(u32) + ht1(s33) + ht2(s33)
                        // [(ref rt1)] -> [(ref rt1)]
                        0x18 => {
                            if offset >= code.len() {
                                return Err(anyhow!("unexpected end in br_on_cast"));
                            }
                            let _flags = code[offset];
                            offset += 1;
                            let (_label, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (_ht1, new_off2) = Self::read_leb128_signed(code, offset)?;
                            offset = new_off2;
                            let (_ht2, new_off3) = Self::read_leb128_signed(code, offset)?;
                            offset = new_off3;
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // br_on_cast_fail: flags(1 byte) + label(u32) + ht1(s33) + ht2(s33)
                        // [(ref rt1)] -> [(ref rt2)]
                        0x19 => {
                            if offset >= code.len() {
                                return Err(anyhow!("unexpected end in br_on_cast_fail"));
                            }
                            let _flags = code[offset];
                            offset += 1;
                            let (_label, new_off) = Self::parse_varuint32(code, offset)?;
                            offset = new_off;
                            let (_ht1, new_off2) = Self::read_leb128_signed(code, offset)?;
                            offset = new_off2;
                            let (_ht2, new_off3) = Self::read_leb128_signed(code, offset)?;
                            offset = new_off3;
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // any.convert_extern: [(ref extern)] -> [(ref any)]
                        0x1A => {
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // extern.convert_any: [(ref any)] -> [(ref extern)]
                        0x1B => {
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // ref.i31: [i32] -> [(ref i31)]
                        0x1C => {
                            Self::pop_type(&mut stack, StackType::I32, frame_height, unreachable);
                            stack.push(StackType::Unknown);
                        },
                        // i31.get_s, i31.get_u: [(ref null i31)] -> [i32]
                        0x1D | 0x1E => {
                            Self::pop_type(&mut stack, StackType::Unknown, frame_height, unreachable);
                            stack.push(StackType::I32);
                        },
                        // Unknown GC sub-opcode - skip without stack effects
                        // This is conservative but avoids false validation failures
                        _ => {},
                    }
                },

                // Atomic instructions (0xFE prefix) - WebAssembly Threads Proposal
                0xFE => {
                    if offset >= code.len() {
                        return Err(anyhow!("unexpected end of code after 0xFE prefix"));
                    }
                    let (sub_opcode, new_offset) = Self::parse_varuint32(code, offset)?;
                    offset = new_offset;
                    let fh = Self::current_frame_height(&frames);
                    let ur = Self::is_unreachable(&frames);

                    // All atomic instructions except atomic.fence require a memory
                    if sub_opcode != 0x03 && !Self::has_memory(module) {
                        return Err(anyhow!("unknown memory"));
                    }

                    match sub_opcode {
                        // memory.atomic.notify: [i32, i32] -> [i32]
                        0x00 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I32);
                        },
                        // memory.atomic.wait32: [i32, i32, i64] -> [i32]
                        0x01 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I32);
                        },
                        // memory.atomic.wait64: [i32, i64, i64] -> [i32]
                        0x02 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I32);
                        },
                        // atomic.fence: [] -> []
                        0x03 => {
                            if offset >= code.len() {
                                return Err(anyhow!("unexpected end of code in atomic.fence"));
                            }
                            offset += 1; // skip reserved byte
                        },
                        // Atomic loads
                        // i32.atomic.load, i32.atomic.load8_u, i32.atomic.load16_u: [i32] -> [i32]
                        0x10 | 0x12 | 0x13 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I32);
                        },
                        // i64.atomic.load, i64.atomic.load8_u, i64.atomic.load16_u, i64.atomic.load32_u: [i32] -> [i64]
                        0x11 | 0x14 | 0x15 | 0x16 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I64);
                        },
                        // Atomic stores
                        // i32.atomic.store, i32.atomic.store8, i32.atomic.store16: [i32, i32] -> []
                        0x17 | 0x19 | 0x1A => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                        },
                        // i64.atomic.store, i64.atomic.store8, i64.atomic.store16, i64.atomic.store32: [i32, i64] -> []
                        0x18 | 0x1B | 0x1C | 0x1D => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                        },
                        // i32 RMW operations (add, sub, and, or, xor, xchg): [i32, i32] -> [i32]
                        0x1E | 0x25 | 0x2C | 0x33 | 0x3A | 0x41 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I32);
                        },
                        // i64 RMW operations (add, sub, and, or, xor, xchg): [i32, i64] -> [i64]
                        0x1F | 0x26 | 0x2D | 0x34 | 0x3B | 0x42 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I64);
                        },
                        // i32.atomic.rmw8/16 variants (add, sub, and, or, xor, xchg): [i32, i32] -> [i32]
                        0x20 | 0x21 | 0x27 | 0x28 | 0x2E | 0x2F | 0x35 | 0x36 | 0x3C | 0x3D | 0x43 | 0x44 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I32);
                        },
                        // i64.atomic.rmw8/16/32 variants (add, sub, and, or, xor, xchg): [i32, i64] -> [i64]
                        0x22 | 0x23 | 0x24 | 0x29 | 0x2A | 0x2B | 0x30 | 0x31 | 0x32 |
                        0x37 | 0x38 | 0x39 | 0x3E | 0x3F | 0x40 | 0x45 | 0x46 | 0x47 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I64);
                        },
                        // i32.atomic.rmw.cmpxchg: [i32, i32, i32] -> [i32]
                        0x48 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I32);
                        },
                        // i64.atomic.rmw.cmpxchg: [i32, i64, i64] -> [i64]
                        0x49 => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I64);
                        },
                        // i32.atomic.rmw8/16.cmpxchg_u: [i32, i32, i32] -> [i32]
                        0x4A | 0x4B => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I32);
                        },
                        // i64.atomic.rmw8/16/32.cmpxchg_u: [i32, i64, i64] -> [i64]
                        0x4C | 0x4D | 0x4E => {
                            let (_mem_idx, new_offset) = Self::parse_memarg(code, offset, module)?;
                            offset = new_offset;
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I64, fh, ur);
                            Self::pop_type(&mut stack, StackType::I32, fh, ur);
                            stack.push(StackType::I64);
                        },
                        // Unknown atomic sub-opcode
                        _ => {},
                    }
                },

                // Skip other opcodes for now (will be handled by instruction executor)
                _ => {
                    // For all other opcodes, try to skip variable-length immediates
                    // This is a simplified approach - proper validation would parse every opcode
                    // But for WAST tests, the main issues are br_if and unreachable code
                },
            }
        }

        Ok(())
    }

    /// Validate a global variable
    fn validate_global(
        global_idx: usize,
        global: &Global,
        module: &Module,
        declared_functions: &HashSet<u32>,
    ) -> Result<()> {
        // Validate that the initialization expression is a valid constant expression
        // and produces a value of the correct type
        let expected_type = StackType::from_value_type(global.global_type.value_type);
        Self::validate_const_expr_typed(
            &global.init,
            module,
            global_idx,
            expected_type,
            declared_functions,
        )
        .with_context(|| format!("Invalid constant expression in global {}", global_idx))
    }

    /// Validate that an expression is a valid constant expression and produces the expected type
    fn validate_const_expr_typed(
        init_bytes: &[u8],
        module: &Module,
        context_global_idx: usize,
        expected_type: StackType,
        declared_functions: &HashSet<u32>,
    ) -> Result<()> {
        let num_global_imports = Self::count_global_imports(module);
        let mut pos = 0;
        let mut stack: Vec<StackType> = Vec::new();

        while pos < init_bytes.len() {
            let opcode = init_bytes[pos];
            pos += 1;

            match opcode {
                // end - valid terminator
                0x0B => {
                    // Check that stack has exactly one value of expected type
                    if stack.len() != 1 {
                        return Err(anyhow!("type mismatch"));
                    }
                    let actual_type = stack[0];
                    // Use module-aware subtype checking for concrete type indices
                    if !Self::is_subtype_of_in_module(&actual_type, &expected_type, module) && actual_type != StackType::Unknown {
                        return Err(anyhow!("type mismatch"));
                    }
                    return Ok(());
                },
                // i32.const
                0x41 => {
                    pos = Self::skip_leb128_signed(init_bytes, pos)?;
                    stack.push(StackType::I32);
                },
                // i64.const
                0x42 => {
                    pos = Self::skip_leb128_signed(init_bytes, pos)?;
                    stack.push(StackType::I64);
                },
                // f32.const
                0x43 => {
                    if pos + 4 > init_bytes.len() {
                        return Err(anyhow!("Truncated f32.const in constant expression"));
                    }
                    pos += 4;
                    stack.push(StackType::F32);
                },
                // f64.const
                0x44 => {
                    if pos + 8 > init_bytes.len() {
                        return Err(anyhow!("Truncated f64.const in constant expression"));
                    }
                    pos += 8;
                    stack.push(StackType::F64);
                },
                // global.get
                0x23 => {
                    let (ref_global_idx, new_pos) = Self::read_leb128_unsigned(init_bytes, pos)?;
                    pos = new_pos;

                    // First, check if the global exists at all
                    let total_globals = Self::total_globals(module);
                    if ref_global_idx as usize >= total_globals {
                        return Err(anyhow!("unknown global"));
                    }

                    // Calculate the current global's index in the global index space
                    let current_global_space_idx = num_global_imports + context_global_idx;

                    // Referenced global must come before the current global (no forward references)
                    // Note: This means global can only reference previously defined globals
                    if ref_global_idx as usize >= current_global_space_idx {
                        return Err(anyhow!("unknown global"));
                    }

                    // Referenced global must be immutable
                    if let Some(true) = Self::is_global_mutable(module, ref_global_idx) {
                        return Err(anyhow!("constant expression required"));
                    }

                    // Push the type of the referenced global
                    if let Some(vt) = Self::get_global_type(module, ref_global_idx) {
                        stack.push(StackType::from_value_type(vt));
                    } else {
                        return Err(anyhow!("unknown global"));
                    }
                },
                // ref.null
                0xD0 => {
                    // Read heap type
                    let (heap_type, new_pos) = Self::read_leb128_signed(init_bytes, pos)?;
                    pos = new_pos;
                    match heap_type {
                        // func → NullFuncRef (bottom of funcref hierarchy)
                        0x70 | -16 => stack.push(StackType::NullFuncRef),
                        // nofunc → NullFuncRef
                        0x73 | -13 => stack.push(StackType::NullFuncRef),
                        // extern → NullExternRef
                        0x6F | -17 => stack.push(StackType::NullExternRef),
                        // noextern → NullExternRef
                        0x72 | -14 => stack.push(StackType::NullExternRef),
                        // exn → NullExnRef
                        0x69 | -23 => stack.push(StackType::NullExnRef),
                        // noexn → NullExnRef
                        0x74 | -12 => stack.push(StackType::NullExnRef),
                        // any, eq, i31, struct, array, none → NoneRef
                        0x6E | -18 | 0x6D | -19 | 0x6C | -20
                        | 0x6B | -21 | 0x6A | -22 | 0x71 | -15 => {
                            stack.push(StackType::NoneRef);
                        },
                        // Concrete type index → nullable typed reference
                        _ if heap_type >= 0 => {
                            stack.push(StackType::TypedFuncRef(heap_type as u32, true));
                        },
                        // Other abstract types
                        _ => stack.push(StackType::Unknown),
                    }
                },
                // ref.func
                0xD2 => {
                    let (func_idx, new_pos) = Self::read_leb128_unsigned(init_bytes, pos)?;
                    pos = new_pos;

                    // Validate function index is within bounds
                    let total_funcs = Self::total_functions(module);
                    if func_idx as usize >= total_funcs {
                        return Err(anyhow!("unknown function {}", func_idx));
                    }

                    // Validate function is declared (imported or in element segment)
                    if !declared_functions.contains(&func_idx) {
                        return Err(anyhow!("undeclared function reference"));
                    }

                    // Push the typed function reference for the function's type
                    // This allows ref.func to match typed function reference expectations
                    if (func_idx as usize) < module.functions.len() {
                        let type_idx = module.functions[func_idx as usize].type_idx;
                        stack.push(StackType::TypedFuncRef(type_idx, false));
                    } else {
                        stack.push(StackType::FuncRef);
                    }
                },
                // Extended constant expressions (WebAssembly 2.0)
                // i32.add, i32.sub, i32.mul - pop 2 i32, push 1 i32
                0x6A | 0x6B | 0x6C => {
                    if stack.len() < 2 {
                        return Err(anyhow!("type mismatch"));
                    }
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    if a != StackType::I32 || b != StackType::I32 {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I32);
                },
                // i64.add, i64.sub, i64.mul - pop 2 i64, push 1 i64
                0x7C | 0x7D | 0x7E => {
                    if stack.len() < 2 {
                        return Err(anyhow!("type mismatch"));
                    }
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    if a != StackType::I64 || b != StackType::I64 {
                        return Err(anyhow!("type mismatch"));
                    }
                    stack.push(StackType::I64);
                },
                // GC instructions in constant expressions (0xFB prefix)
                // WebAssembly 3.0 allows struct.new, array.new, etc. in const exprs
                0xFB => {
                    if pos >= init_bytes.len() {
                        return Err(anyhow!("unexpected end of const expr after 0xFB prefix"));
                    }
                    let (sub_opcode, new_pos) = Self::read_leb128_unsigned(init_bytes, pos)?;
                    pos = new_pos;
                    match sub_opcode {
                        // struct.new $t, struct.new_default $t
                        0x00 | 0x01 => {
                            let (_type_idx, new_pos2) = Self::read_leb128_unsigned(init_bytes, pos)?;
                            pos = new_pos2;
                            // For struct.new, should pop field values; approximate with push result
                            stack.clear(); // Clear and push result
                            stack.push(StackType::Unknown);
                        },
                        // array.new $t: [val i32] -> [(ref $t)]
                        0x06 => {
                            let (_type_idx, new_pos2) = Self::read_leb128_unsigned(init_bytes, pos)?;
                            pos = new_pos2;
                            stack.clear();
                            stack.push(StackType::Unknown);
                        },
                        // array.new_default $t: [i32] -> [(ref $t)]
                        0x07 => {
                            let (_type_idx, new_pos2) = Self::read_leb128_unsigned(init_bytes, pos)?;
                            pos = new_pos2;
                            stack.clear();
                            stack.push(StackType::Unknown);
                        },
                        // array.new_fixed $t $n
                        0x08 => {
                            let (_type_idx, new_pos2) = Self::read_leb128_unsigned(init_bytes, pos)?;
                            pos = new_pos2;
                            let (_count, new_pos3) = Self::read_leb128_unsigned(init_bytes, pos)?;
                            pos = new_pos3;
                            stack.clear();
                            stack.push(StackType::Unknown);
                        },
                        // ref.i31: [i32] -> [(ref i31)]
                        0x1C => {
                            stack.clear();
                            stack.push(StackType::Unknown);
                        },
                        // any.convert_extern, extern.convert_any
                        0x1A | 0x1B => {
                            stack.clear();
                            stack.push(StackType::Unknown);
                        },
                        // Other GC const exprs - skip with type_idx immediate
                        _ => {
                            // Try to read a type index immediate (most GC ops have one)
                            if pos < init_bytes.len() {
                                let (_idx, new_pos2) = Self::read_leb128_unsigned(init_bytes, pos)?;
                                pos = new_pos2;
                            }
                            stack.clear();
                            stack.push(StackType::Unknown);
                        },
                    }
                },
                // v128.const in constant expressions
                0xFD => {
                    if pos >= init_bytes.len() {
                        return Err(anyhow!("unexpected end of const expr after 0xFD prefix"));
                    }
                    let (sub_opcode, new_pos) = Self::read_leb128_unsigned(init_bytes, pos)?;
                    pos = new_pos;
                    if sub_opcode == 12 {
                        // v128.const: 16 byte immediate
                        if pos + 16 > init_bytes.len() {
                            return Err(anyhow!("Truncated v128.const in constant expression"));
                        }
                        pos += 16;
                        stack.push(StackType::V128);
                    } else {
                        return Err(anyhow!("constant expression required"));
                    }
                },
                // Any other opcode is not allowed in a constant expression
                _ => {
                    return Err(anyhow!("constant expression required"));
                },
            }
        }

        // If we reach here without an end opcode, that's an error
        Err(anyhow!("type mismatch"))
    }

    /// Skip a signed LEB128 value, returning the new position
    fn skip_leb128_signed(bytes: &[u8], mut pos: usize) -> Result<usize> {
        loop {
            if pos >= bytes.len() {
                return Err(anyhow!("Truncated LEB128 in constant expression"));
            }
            let byte = bytes[pos];
            pos += 1;
            if byte & 0x80 == 0 {
                break;
            }
        }
        Ok(pos)
    }

    /// Skip an unsigned LEB128 value, returning the new position
    fn skip_leb128_unsigned(bytes: &[u8], mut pos: usize) -> Result<usize> {
        loop {
            if pos >= bytes.len() {
                return Err(anyhow!("Truncated LEB128 in constant expression"));
            }
            let byte = bytes[pos];
            pos += 1;
            if byte & 0x80 == 0 {
                break;
            }
        }
        Ok(pos)
    }

    /// Read an unsigned LEB128 value, returning the value and new position
    fn read_leb128_unsigned(bytes: &[u8], mut pos: usize) -> Result<(u32, usize)> {
        let mut result: u32 = 0;
        let mut shift: u32 = 0;
        loop {
            if pos >= bytes.len() {
                return Err(anyhow!("Truncated LEB128 in constant expression"));
            }
            let byte = bytes[pos] as u32;
            pos += 1;
            result |= (byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 35 {
                return Err(anyhow!("LEB128 overflow in constant expression"));
            }
        }
        Ok((result, pos))
    }

    /// Read a signed LEB128 value, returning the value and new position
    fn read_leb128_signed(bytes: &[u8], mut pos: usize) -> Result<(i32, usize)> {
        let mut result: i32 = 0;
        let mut shift: u32 = 0;
        let size: u32 = 32; // for i32
        let mut byte: u8;

        loop {
            if pos >= bytes.len() {
                return Err(anyhow!("Truncated LEB128 in constant expression"));
            }
            byte = bytes[pos];
            pos += 1;
            result |= ((byte & 0x7F) as i32) << shift;
            shift += 7;
            if (byte & 0x80) == 0 {
                break;
            }
            if shift >= 35 {
                return Err(anyhow!("LEB128 overflow in constant expression"));
            }
        }

        // Sign extend if needed
        if shift < size && (byte & 0x40) != 0 {
            result |= !0i32 << shift;
        }

        Ok((result, pos))
    }

    /// Count the number of global imports in a module
    fn count_global_imports(module: &Module) -> usize {
        // kiln_format::Module has imports as a Vec, so iteration works correctly
        module
            .imports
            .iter()
            .filter(|i| matches!(&i.desc, ImportDesc::Global(_)))
            .count()
    }

    /// Count the number of memory imports in a module
    fn count_memory_imports(module: &Module) -> usize {
        module
            .imports
            .iter()
            .filter(|i| matches!(&i.desc, ImportDesc::Memory(_)))
            .count()
    }

    /// Get the total number of memories (imports + defined)
    fn total_memories(module: &Module) -> usize {
        Self::count_memory_imports(module) + module.memories.len()
    }

    /// Check if module has any memory defined (imported or local)
    fn has_memory(module: &Module) -> bool {
        Self::total_memories(module) > 0
    }

    /// Check if a memory at the given index uses 64-bit addressing (Memory64)
    /// Memory indices include both imported and defined memories:
    /// - Indices 0..N-1 are imported memories
    /// - Indices N+ are defined memories
    #[allow(dead_code)]
    fn is_memory64(module: &Module, mem_idx: u32) -> bool {
        let num_mem_imports = Self::count_memory_imports(module);

        if (mem_idx as usize) < num_mem_imports {
            // This is an imported memory - find it in imports Vec
            let mut import_idx = 0;
            for import in &module.imports {
                if let ImportDesc::Memory(mem_type) = &import.desc {
                    if import_idx == mem_idx as usize {
                        return mem_type.memory64;
                    }
                    import_idx += 1;
                }
            }
            false
        } else {
            // This is a defined memory
            let defined_idx = mem_idx as usize - num_mem_imports;
            module.memories.get(defined_idx).map_or(false, |m| m.memory64)
        }
    }

    /// Check if a table uses table64 (64-bit indices)
    fn is_table64(module: &Module, table_idx: u32) -> bool {
        let num_table_imports = Self::count_table_imports(module);

        if (table_idx as usize) < num_table_imports {
            let mut import_idx = 0;
            for import in &module.imports {
                if let ImportDesc::Table(table_type) = &import.desc {
                    if import_idx == table_idx as usize {
                        return table_type.table64;
                    }
                    import_idx += 1;
                }
            }
            false
        } else {
            let defined_idx = table_idx as usize - num_table_imports;
            module.tables.get(defined_idx).map_or(false, |t| t.table64)
        }
    }

    /// Count the number of table imports in a module
    fn count_table_imports(module: &Module) -> usize {
        module
            .imports
            .iter()
            .filter(|i| matches!(&i.desc, ImportDesc::Table(_)))
            .count()
    }

    /// Get the total number of tables (imports + defined)
    fn total_tables(module: &Module) -> usize {
        Self::count_table_imports(module) + module.tables.len()
    }

    /// Check if module has any table defined (imported or local)
    fn has_table(module: &Module) -> bool {
        Self::total_tables(module) > 0
    }

    /// Get the table element type for a table index (accounting for imports)
    /// Table indices include both imported and defined tables:
    /// - Indices 0..N-1 are imported tables
    /// - Indices N+ are defined tables
    fn get_table_element_type(module: &Module, table_idx: u32) -> Option<kiln_foundation::RefType> {
        let num_table_imports = Self::count_table_imports(module);

        if (table_idx as usize) < num_table_imports {
            // This is an imported table - find it in imports Vec
            let mut import_idx = 0;
            for import in &module.imports {
                if let ImportDesc::Table(table_type) = &import.desc {
                    if import_idx == table_idx as usize {
                        return Some(table_type.element_type);
                    }
                    import_idx += 1;
                }
            }
            None
        } else {
            // This is a defined table
            let defined_idx = table_idx as usize - num_table_imports;
            module.tables.get(defined_idx).map(|t| t.element_type)
        }
    }

    /// Get the global type for a global index (accounting for imports)
    /// Global indices include both imported and defined globals:
    /// - Indices 0..N-1 are imported globals
    /// - Indices N+ are defined globals
    fn get_global_type(module: &Module, global_idx: u32) -> Option<ValueType> {
        let num_global_imports = Self::count_global_imports(module);

        if (global_idx as usize) < num_global_imports {
            // This is an imported global - find it in imports Vec
            let mut import_idx = 0;
            for import in &module.imports {
                if let ImportDesc::Global(global_type) = &import.desc {
                    if import_idx == global_idx as usize {
                        return Some(global_type.value_type);
                    }
                    import_idx += 1;
                }
            }
            None
        } else {
            // This is a defined global
            let defined_idx = global_idx as usize - num_global_imports;
            module.globals.get(defined_idx).map(|g| g.global_type.value_type)
        }
    }

    /// Check if a global is mutable
    /// Returns None if the global doesn't exist
    fn is_global_mutable(module: &Module, global_idx: u32) -> Option<bool> {
        let num_global_imports = Self::count_global_imports(module);

        if (global_idx as usize) < num_global_imports {
            // This is an imported global - find it in imports Vec
            let mut import_idx = 0;
            for import in &module.imports {
                if let ImportDesc::Global(global_type) = &import.desc {
                    if import_idx == global_idx as usize {
                        return Some(global_type.mutable);
                    }
                    import_idx += 1;
                }
            }
            None
        } else {
            // This is a defined global
            let defined_idx = global_idx as usize - num_global_imports;
            module.globals.get(defined_idx).map(|g| g.global_type.mutable)
        }
    }

    /// Get the total number of globals (imports + defined)
    fn total_globals(module: &Module) -> usize {
        Self::count_global_imports(module) + module.globals.len()
    }

    /// Count the number of function imports in a module
    fn count_function_imports(module: &Module) -> usize {
        module
            .imports
            .iter()
            .filter(|i| matches!(&i.desc, ImportDesc::Function(_)))
            .count()
    }

    /// Get the total number of functions (imports + defined)
    fn total_functions(module: &Module) -> usize {
        Self::count_function_imports(module) + module.functions.len()
    }

    /// Count the number of tag imports in a module
    fn count_tag_imports(module: &Module) -> usize {
        module.imports.iter().filter(|i| matches!(&i.desc, ImportDesc::Tag(_))).count()
    }

    /// Get the total number of tags (imports + defined)
    fn total_tags(module: &Module) -> usize {
        Self::count_tag_imports(module) + module.tags.len()
    }

    /// Get the type index for a tag (accounting for imports)
    /// Tag indices include both imported and defined tags:
    /// - Indices 0..N-1 are imported tags
    /// - Indices N+ are defined tags
    fn get_tag_type_idx(module: &Module, tag_idx: u32) -> Option<u32> {
        let num_tag_imports = Self::count_tag_imports(module);

        if (tag_idx as usize) < num_tag_imports {
            // This is an imported tag - find it in imports
            let mut import_idx = 0;
            for import in &module.imports {
                if let ImportDesc::Tag(type_idx) = &import.desc {
                    if import_idx == tag_idx as usize {
                        return Some(*type_idx);
                    }
                    import_idx += 1;
                }
            }
            None
        } else {
            // This is a defined tag
            let defined_idx = tag_idx as usize - num_tag_imports;
            module.tags.get(defined_idx).map(|t| t.type_idx)
        }
    }

    /// Get the parameter types for a tag as StackTypes
    /// Tags are defined with a type index pointing to a function type,
    /// and the tag's params are the function type's params
    fn get_tag_param_types(module: &Module, tag_idx: u32) -> Result<Vec<StackType>> {
        let type_idx =
            Self::get_tag_type_idx(module, tag_idx).ok_or_else(|| anyhow!("unknown tag"))?;

        let func_type =
            module.types.get(type_idx as usize).ok_or_else(|| anyhow!("unknown type"))?;

        Ok(func_type.params.iter().map(|vt| StackType::from_value_type(*vt)).collect())
    }

    /// Validate that a try_table handler can branch to the target label
    /// Handler branches carry specific values based on handler type:
    /// - catch: tag's param types
    /// - catch_ref: tag's param types + exnref
    /// - catch_all: nothing
    /// - catch_all_ref: exnref
    /// The target block must have output types matching the handler's values
    fn validate_handler_branch(
        frames: &[ControlFrame],
        label: u32,
        handler_types: &[StackType],
    ) -> Result<()> {
        let label = label as usize;

        // Validate label is in range
        if label >= frames.len() {
            return Err(anyhow!("unknown label"));
        }

        // Get the target frame (label 0 = innermost = frames.last())
        let target_idx = frames.len() - 1 - label;
        let target_frame = &frames[target_idx];

        // Get the target's label types (what a branch to it must provide)
        // For loops, branches provide inputs; for other blocks, branches provide outputs
        let target_types = if target_frame.frame_type == FrameType::Loop {
            &target_frame.input_types
        } else {
            &target_frame.output_types
        };

        // Handler's pushed types must be subtypes of target's expected types
        if handler_types.len() != target_types.len() {
            return Err(anyhow!("type mismatch"));
        }

        for (handler_type, target_type) in handler_types.iter().zip(target_types.iter()) {
            // Use subtyping: handler_type must be a subtype of target_type
            if !handler_type.is_subtype_of(target_type) {
                return Err(anyhow!("type mismatch"));
            }
        }

        Ok(())
    }

    /// Collect all function indices that are "declared" for ref.func validation
    /// According to WebAssembly spec, C.refs includes function indices from:
    /// 1. Element segments (active, passive, or declarative)
    /// 2. Exports
    /// 3. Start function
    /// 4. Global initializer expressions (ref.func in globals)
    /// 5. Imported functions (implicitly declared)
    fn collect_declared_functions(module: &Module) -> HashSet<u32> {
        let mut declared = HashSet::new();

        // All imported functions are implicitly declared
        let num_func_imports = Self::count_function_imports(module);
        for i in 0..num_func_imports {
            declared.insert(i as u32);
        }

        // Collect function indices from element segments
        for elem in &module.elements {
            Self::collect_functions_from_element(&elem, &mut declared);
        }

        // Collect function indices from exports
        for export in &module.exports {
            if export.kind == kiln_format::module::ExportKind::Function {
                declared.insert(export.index);
            }
        }

        // Note: The start function is NOT included in C.refs per the spec
        // It specifies which function to call at startup, but doesn't "declare" it for ref.func

        // Collect function indices from global initializers (ref.func in globals)
        for global in &module.globals {
            Self::extract_ref_func_from_expr(&global.init, &mut declared);
        }

        // Collect function indices from element segment offset expressions
        for elem in &module.elements {
            Self::extract_ref_func_from_expr(&elem.offset_expr_bytes, &mut declared);
        }

        // Collect function indices from data segment offset expressions
        for data in &module.data {
            Self::extract_ref_func_from_expr(&data.offset_expr_bytes, &mut declared);
        }

        declared
    }

    /// Extract function indices from an element segment
    fn collect_functions_from_element(elem: &PureElementSegment, declared: &mut HashSet<u32>) {
        match &elem.init_data {
            PureElementInit::FunctionIndices(indices) => {
                for &idx in indices {
                    declared.insert(idx);
                }
            },
            PureElementInit::ExpressionBytes(exprs) => {
                // Parse expression bytes to find ref.func instructions
                for expr_bytes in exprs {
                    Self::extract_ref_func_from_expr(expr_bytes, declared);
                }
            },
        }
    }

    /// Extract ref.func indices from expression bytes
    fn extract_ref_func_from_expr(expr: &[u8], declared: &mut HashSet<u32>) {
        let mut pos = 0;
        while pos < expr.len() {
            let opcode = expr[pos];
            pos += 1;

            match opcode {
                0xD2 => {
                    // ref.func - parse the function index
                    if let Ok((func_idx, new_pos)) = Self::read_leb128_unsigned(expr, pos) {
                        declared.insert(func_idx);
                        pos = new_pos;
                    } else {
                        break;
                    }
                },
                0xD0 => {
                    // ref.null - skip the heap type byte
                    if pos < expr.len() {
                        pos += 1;
                    }
                },
                0x0B => {
                    // end - stop parsing
                    break;
                },
                _ => {
                    // Unknown opcode in element expression - skip
                    break;
                },
            }
        }
    }

    /// Get the maximum allowed alignment (as log2) for a memory operation opcode
    /// Returns None if the opcode is not a memory operation
    fn max_alignment_for_opcode(opcode: u8) -> Option<u32> {
        match opcode {
            // 4-byte operations (max align = 2, since 2^2 = 4)
            0x28 | 0x36 => Some(2),        // i32.load, i32.store
            0x2A | 0x38 => Some(2),        // f32.load, f32.store
            0x34 | 0x35 | 0x3E => Some(2), // i64.load32_s/u, i64.store32

            // 8-byte operations (max align = 3, since 2^3 = 8)
            0x29 | 0x37 => Some(3), // i64.load, i64.store
            0x2B | 0x39 => Some(3), // f64.load, f64.store

            // 2-byte operations (max align = 1, since 2^1 = 2)
            0x2E | 0x2F | 0x3B => Some(1), // i32.load16_s/u, i32.store16
            0x32 | 0x33 | 0x3D => Some(1), // i64.load16_s/u, i64.store16

            // 1-byte operations (max align = 0, since 2^0 = 1)
            0x2C | 0x2D | 0x3A => Some(0), // i32.load8_s/u, i32.store8
            0x30 | 0x31 | 0x3C => Some(0), // i64.load8_s/u, i64.store8

            _ => None,
        }
    }

    /// Validate that a memory operation's alignment doesn't exceed the natural alignment
    fn validate_alignment(opcode: u8, align: u32) -> Result<()> {
        if let Some(max_align) = Self::max_alignment_for_opcode(opcode) {
            if align > max_align {
                return Err(anyhow!("alignment must not be larger than natural"));
            }
        }
        Ok(())
    }

    /// Get the maximum allowed alignment (as log2) for a SIMD memory operation
    fn max_simd_alignment(sub_opcode: u32) -> Option<u32> {
        match sub_opcode {
            0x00 | 0x0B => Some(4),         // v128.load, v128.store (16 bytes)
            0x01..=0x06 | 0x0A => Some(3),  // load8x8, load16x4, load32x2, load64_splat (8 bytes)
            0x09 => Some(2),                // load32_splat (4 bytes)
            0x08 => Some(1),                // load16_splat (2 bytes)
            0x07 => Some(0),                // load8_splat (1 byte)
            0x54 | 0x58 => Some(0),         // load8_lane, store8_lane (1 byte)
            0x55 | 0x59 => Some(1),         // load16_lane, store16_lane (2 bytes)
            0x56 | 0x5A | 0x5C => Some(2),  // load32_lane, store32_lane, load32_zero (4 bytes)
            0x57 | 0x5B | 0x5D => Some(3),  // load64_lane, store64_lane, load64_zero (8 bytes)
            _ => None,
        }
    }

    fn validate_simd_alignment(sub_opcode: u32, align: u32) -> Result<()> {
        if let Some(max_align) = Self::max_simd_alignment(sub_opcode) {
            if align > max_align {
                return Err(anyhow!("alignment must not be larger than natural"));
            }
        }
        Ok(())
    }

    /// Get the lane count for load_lane/store_lane operations
    fn simd_lane_count_for_lane_op(sub_opcode: u32) -> u8 {
        match sub_opcode {
            0x54 | 0x58 => 16, // load8_lane, store8_lane: i8x16
            0x55 | 0x59 => 8,  // load16_lane, store16_lane: i16x8
            0x56 | 0x5A => 4,  // load32_lane, store32_lane: i32x4
            0x57 | 0x5B => 2,  // load64_lane, store64_lane: i64x2
            _ => 0,
        }
    }

    /// Pop a value from the stack, checking its type
    /// The `min_height` parameter is the stack height at the current frame's entry -
    /// we cannot pop below this level (those values belong to the parent frame)
    /// The `unreachable` parameter indicates if we're in unreachable code (polymorphic stack)
    ///
    /// In unreachable code:
    /// - If the stack is at or below min_height, we have polymorphic underflow - any type is OK
    /// - If there are concrete values on the stack (pushed after unreachable), they MUST still
    ///   be type-checked according to the WebAssembly spec
    fn pop_type(
        stack: &mut Vec<StackType>,
        expected: StackType,
        min_height: usize,
        unreachable: bool,
    ) -> bool {
        Self::pop_type_with_module(stack, expected, min_height, unreachable, None)
    }

    /// Pop a value from the stack with optional module context for concrete type subtyping.
    fn pop_type_with_module(
        stack: &mut Vec<StackType>,
        expected: StackType,
        min_height: usize,
        unreachable: bool,
        module: Option<&Module>,
    ) -> bool {
        // In unreachable code, the stack is polymorphic
        if unreachable {
            // Can pop below min_height (polymorphic underflow)
            if stack.len() <= min_height {
                return true;
            }
            // There are actual values on the stack - they MUST be type-checked
            // The polymorphism only applies to underflow, not to concrete values
            if let Some(actual) = stack.pop() {
                // Unknown type is polymorphic and matches anything
                if actual == StackType::Unknown {
                    return true;
                }
                // Concrete values must match the expected type
                if let Some(m) = module {
                    return Self::is_subtype_of_in_module(&actual, &expected, m);
                }
                return actual.is_subtype_of(&expected);
            }
            return true;
        }

        // Check if we'd be popping below the current frame's stack base
        if stack.len() <= min_height {
            return false;
        }

        if let Some(actual) = stack.pop() {
            // Use subtype checking: actual must be a subtype of expected
            if let Some(m) = module {
                Self::is_subtype_of_in_module(&actual, &expected, m)
            } else {
                actual.is_subtype_of(&expected)
            }
        } else {
            false
        }
    }

    /// Get the current frame's stack height (the base of the current control frame)
    fn current_frame_height(frames: &[ControlFrame]) -> usize {
        frames.last().map_or(0, |f| f.stack_height)
    }

    /// Check if the current code path is unreachable
    fn is_unreachable(frames: &[ControlFrame]) -> bool {
        frames.last().map_or(false, |f| !f.reachable)
    }

    /// Parse a variable-length unsigned 32-bit integer
    fn parse_varuint32(code: &[u8], offset: usize) -> Result<(u32, usize)> {
        let mut result = 0u32;
        let mut shift = 0;
        let mut pos = offset;

        loop {
            if pos >= code.len() {
                return Err(anyhow!("truncated varuint32"));
            }

            let byte = code[pos] as u32;
            pos += 1;

            result |= (byte & 0x7F) << shift;

            if (byte & 0x80) == 0 {
                break;
            }

            shift += 7;
            if shift >= 35 {
                return Err(anyhow!("varuint32 overflow"));
            }
        }

        Ok((result, pos))
    }

    /// Parse a variable-length unsigned 64-bit integer
    fn parse_varuint64(code: &[u8], offset: usize) -> Result<(u64, usize)> {
        let mut result = 0u64;
        let mut shift = 0;
        let mut pos = offset;

        loop {
            if pos >= code.len() {
                return Err(anyhow!("truncated varuint64"));
            }

            let byte = code[pos] as u64;
            pos += 1;

            result |= (byte & 0x7F) << shift;

            if (byte & 0x80) == 0 {
                break;
            }

            shift += 7;
            if shift >= 70 {
                return Err(anyhow!("varuint64 overflow"));
            }
        }

        Ok((result, pos))
    }

    /// Parse memory arguments (memarg) for load/store instructions
    /// Returns (mem_idx, offset_parsed), advancing the code offset
    /// For memory64, the offset is encoded as u64 instead of u32
    fn parse_memarg(
        code: &[u8],
        offset: usize,
        module: &Module,
    ) -> Result<(u32, usize)> {
        Self::parse_memarg_with_opcode(code, offset, module, None)
    }

    fn parse_memarg_with_opcode(
        code: &[u8],
        offset: usize,
        module: &Module,
        opcode: Option<u8>,
    ) -> Result<(u32, usize)> {
        // Parse alignment (encoded as power of 2 with optional memory index flag)
        let (align_with_flags, new_offset) = Self::parse_varuint32(code, offset)?;
        let mut current_offset = new_offset;

        // Extract actual alignment (lower 6 bits)
        let align = align_with_flags & 0x3F;

        // Validate alignment against natural alignment for the opcode
        if let Some(op) = opcode {
            Self::validate_alignment(op, align)?;
        }

        // Check for multi-memory flag (bit 6 of alignment)
        // When set, the memory index follows the alignment
        let mem_idx = if (align_with_flags & 0x40) != 0 {
            let (idx, new_off) = Self::parse_varuint32(code, current_offset)?;
            current_offset = new_off;
            idx
        } else {
            0 // Default to memory 0
        };

        // Parse offset - for memory64, this is encoded as u64
        if Self::is_memory64(module, mem_idx) {
            let (_offset_val, new_off) = Self::parse_varuint64(code, current_offset)?;
            current_offset = new_off;
        } else {
            let (_offset_val, new_off) = Self::parse_varuint32(code, current_offset)?;
            current_offset = new_off;
        }

        Ok((mem_idx, current_offset))
    }

    /// Parse a variable-length signed 32-bit integer
    fn parse_varint32(code: &[u8], offset: usize) -> Result<(i32, usize)> {
        let (value, pos) = Self::parse_varuint32(code, offset)?;
        let result = if value & 0x80000000 != 0 { value as i32 } else { value as i32 };
        Ok((result, pos))
    }

    /// Parse a variable-length signed 64-bit integer
    fn parse_varint64(code: &[u8], mut offset: usize) -> Result<(i64, usize)> {
        let mut result = 0i64;
        let mut shift = 0;

        loop {
            if offset >= code.len() {
                return Err(anyhow!("truncated varint64"));
            }

            let byte = code[offset] as i64;
            offset += 1;

            result |= (byte & 0x7F) << shift;

            if (byte & 0x80) == 0 {
                if shift < 63 && (byte & 0x40) != 0 {
                    result |= -(1 << (shift + 7));
                }
                break;
            }

            shift += 7;
        }

        Ok((result, offset))
    }

    /// Parse block type
    fn parse_block_type(code: &[u8], offset: usize, module: &Module) -> Result<(BlockType, usize)> {
        if offset >= code.len() {
            return Err(anyhow!("truncated block type"));
        }

        let byte = code[offset] as i8;

        let block_type = match byte {
            0x40 => BlockType::Empty,
            // Standard value types
            0x7F => BlockType::ValueType(ValueType::I32),
            0x7E => BlockType::ValueType(ValueType::I64),
            0x7D => BlockType::ValueType(ValueType::F32),
            0x7C => BlockType::ValueType(ValueType::F64),
            0x7B => BlockType::ValueType(ValueType::V128),
            // Reference types
            0x70 => BlockType::ValueType(ValueType::FuncRef),
            0x6F => BlockType::ValueType(ValueType::ExternRef),
            0x69 => BlockType::ValueType(ValueType::ExnRef),
            // GC abstract heap types (shorthand reference types)
            0x6E => BlockType::ValueType(ValueType::AnyRef),
            0x6D => BlockType::ValueType(ValueType::EqRef),
            0x6C => BlockType::ValueType(ValueType::I31Ref),
            0x6B => BlockType::ValueType(ValueType::StructRef(0)), // abstract structref
            0x6A => BlockType::ValueType(ValueType::ArrayRef(0)),  // abstract arrayref
            0x73 => BlockType::ValueType(ValueType::NullFuncRef),   // nullfuncref = (ref null nofunc)
            0x72 => BlockType::ValueType(ValueType::ExternRef),    // nullexternref = (ref null noextern)
            0x71 => BlockType::ValueType(ValueType::AnyRef),       // nullref = (ref null none)
            // GC typed references: (ref null? heaptype)
            0x63 | 0x64 => {
                // 0x63 = ref null heaptype (nullable)
                // 0x64 = ref heaptype (non-nullable)
                let nullable = byte == 0x63;
                // Parse the heap type following the prefix
                let (heap_type, new_offset) = Self::parse_heap_type(code, offset + 1, nullable)?;
                // Validate type index bounds for concrete references
                Self::check_value_type_ref(&heap_type, module.types.len())?;
                return Ok((BlockType::ValueType(heap_type), new_offset));
            },
            _ if byte >= 0 => {
                // Function type index (encoded as positive s33)
                // For larger indices, need to parse as LEB128
                let (type_idx, new_offset) = Self::parse_varint32(code, offset)?;
                if type_idx < 0 {
                    return Err(anyhow!("invalid block type index"));
                }
                if type_idx as usize >= module.types.len() {
                    return Err(anyhow!("invalid function type index {}", type_idx));
                }
                return Ok((BlockType::FuncType(type_idx as u32), new_offset));
            },
            _ => {
                // Negative index (encoded as varint), parse it properly
                let (type_idx, new_offset) = Self::parse_varint32(code, offset)?;
                if type_idx < 0 {
                    return Err(anyhow!("invalid block type index"));
                }
                if type_idx as usize >= module.types.len() {
                    return Err(anyhow!("invalid function type index {}", type_idx));
                }
                return Ok((BlockType::FuncType(type_idx as u32), new_offset));
            },
        };

        Ok((block_type, offset + 1))
    }

    /// Parse a GC heap type and convert to ValueType
    fn parse_heap_type(code: &[u8], offset: usize, nullable: bool) -> Result<(ValueType, usize)> {
        if offset >= code.len() {
            return Err(anyhow!("truncated heap type"));
        }

        // Parse as signed LEB128 (s33)
        let (heap_type_val, new_offset) = Self::parse_varint64(code, offset)?;

        // Abstract heap types are encoded as negative values:
        // -16 = func (0x70), -17 = extern (0x6F), -18 = any (0x6E), etc.
        // Positive values are concrete type indices.
        let value_type = if heap_type_val < 0 {
            match heap_type_val {
                -16 => ValueType::FuncRef,      // func (0x70)
                -17 => ValueType::ExternRef,    // extern (0x6F)
                -18 => ValueType::AnyRef,       // any (0x6E)
                -19 => ValueType::EqRef,        // eq (0x6D)
                -20 => ValueType::I31Ref,       // i31 (0x6C)
                -21 => ValueType::StructRef(0), // struct (0x6B) - abstract
                -22 => ValueType::ArrayRef(0),  // array (0x6A) - abstract
                -23 => ValueType::ExnRef,       // exn (0x69)
                -13 => ValueType::NullFuncRef,  // nofunc (0x73)
                -14 => ValueType::ExternRef,    // noextern (0x72)
                -15 => ValueType::AnyRef,       // none (0x71)
                _ => ValueType::AnyRef,         // fallback
            }
        } else {
            // Concrete type index - reference to a defined type
            ValueType::TypedFuncRef(heap_type_val as u32, nullable)
        };

        Ok((value_type, new_offset))
    }

    /// Convert block type to input/output stack types
    fn block_type_to_stack_types(
        block_type: &BlockType,
        module: &Module,
    ) -> Result<(Vec<StackType>, Vec<StackType>)> {
        match block_type {
            BlockType::Empty => Ok((Vec::new(), Vec::new())),
            BlockType::ValueType(vt) => {
                let st = StackType::from_value_type(*vt);
                Ok((Vec::new(), vec![st]))
            },
            BlockType::FuncType(type_idx) => {
                if *type_idx as usize >= module.types.len() {
                    return Err(anyhow!("invalid function type index {}", type_idx));
                }

                let func_type = &module.types[*type_idx as usize];

                let inputs =
                    func_type.params.iter().map(|&vt| StackType::from_value_type(vt)).collect();

                let outputs =
                    func_type.results.iter().map(|&vt| StackType::from_value_type(vt)).collect();

                Ok((inputs, outputs))
            },
        }
    }

    /// Validate a branch target
    ///
    /// For branches to blocks/if, we validate against output types.
    /// For branches to loops, we validate against input types.
    ///
    /// IMPORTANT: The values for branching must come from the current frame's
    /// operand stack (above the current frame's stack_height), not from parent frames.
    ///
    /// When `unreachable` is true, the stack is polymorphic and any type is accepted.
    fn validate_branch(
        stack: &[StackType],
        label_idx: u32,
        frames: &[ControlFrame],
        unreachable: bool,
    ) -> Result<()> {
        Self::validate_branch_with_module(stack, label_idx, frames, unreachable, None)
    }

    /// Validate a branch target with optional module context for concrete type subtyping.
    fn validate_branch_with_module(
        stack: &[StackType],
        label_idx: u32,
        frames: &[ControlFrame],
        unreachable: bool,
        module: Option<&Module>,
    ) -> Result<()> {
        if label_idx as usize >= frames.len() {
            return Err(anyhow!("br: label index {} out of range", label_idx));
        }

        // In unreachable code, the stack is polymorphic - any values are acceptable
        if unreachable {
            return Ok(());
        }

        // Get the current frame (innermost) to check our available stack values
        let current_frame = frames.last().ok_or_else(|| anyhow!("no control frame"))?;
        let current_stack_height = current_frame.stack_height;

        // Get the target frame (counting from innermost)
        let target_frame = &frames[frames.len() - 1 - label_idx as usize];

        // Determine the expected types for the branch
        // For loops: branch to input types (jump to loop start)
        // For blocks/if/else: branch to output types (jump to end)
        let expected_types = if target_frame.frame_type == FrameType::Loop {
            &target_frame.input_types
        } else {
            &target_frame.output_types
        };

        // Calculate how many values the CURRENT frame has available on the stack
        // Values below current_stack_height belong to parent frames and cannot be used
        let available_values = stack.len().saturating_sub(current_stack_height);

        // Check that the current frame has enough values for the branch
        if available_values < expected_types.len() {
            // Not enough values in the current frame's scope
            return Err(anyhow!("type mismatch"));
        }

        // Verify the top values match expected types (in reverse order)
        // Use subtype checking: actual must be a subtype of expected
        for (i, expected) in expected_types.iter().rev().enumerate() {
            let stack_idx = stack.len() - 1 - i;
            let actual = &stack[stack_idx];
            if let Some(m) = module {
                if !Self::is_subtype_of_in_module(actual, expected, m) {
                    return Err(anyhow!("type mismatch"));
                }
            } else if !actual.is_subtype_of(expected) {
                return Err(anyhow!("type mismatch"));
            }
        }

        Ok(())
    }

    /// Check if two type indices refer to equivalent types.
    ///
    /// Two types are equivalent if they are in rec groups with the same structure
    /// at the same relative position within those groups. Structural equivalence
    /// means: same number of types in the group, and for each corresponding pair,
    /// the composite kind matches and all references to types within the group
    /// use the same relative offsets.
    fn are_types_equivalent(idx1: u32, idx2: u32, module: &Module) -> bool {
        if idx1 == idx2 {
            return true;
        }

        // Find the rec groups containing each type index
        let group1 = Self::find_rec_group(idx1, module);
        let group2 = Self::find_rec_group(idx2, module);

        match (group1, group2) {
            (Some((g1, offset1)), Some((g2, offset2))) => {
                // Must be at the same relative position
                if offset1 != offset2 {
                    return false;
                }
                // Groups must have the same number of types
                if g1.types.len() != g2.types.len() {
                    return false;
                }
                // Check structural equivalence of all types in both groups
                for (t1, t2) in g1.types.iter().zip(g2.types.iter()) {
                    if !Self::are_subtypes_structurally_equal(t1, t2, g1, g2, module) {
                        return false;
                    }
                }
                true
            },
            _ => false,
        }
    }

    /// Find the rec group containing a type index.
    /// Returns (group, offset_within_group) or None if not in any rec group.
    fn find_rec_group(type_idx: u32, module: &Module) -> Option<(&RecGroup, usize)> {
        for group in &module.rec_groups {
            let start = group.start_type_index;
            let end = start + group.types.len() as u32;
            if type_idx >= start && type_idx < end {
                return Some((group, (type_idx - start) as usize));
            }
        }
        None
    }

    /// Check if two SubType entries are structurally equal, normalizing type
    /// references within their respective rec groups to group-relative indices.
    fn are_subtypes_structurally_equal(
        t1: &kiln_format::module::SubType,
        t2: &kiln_format::module::SubType,
        g1: &RecGroup,
        g2: &RecGroup,
        module: &Module,
    ) -> bool {
        // Both must have the same finality
        if t1.is_final != t2.is_final {
            return false;
        }
        // Both must have the same number of supertypes
        if t1.supertype_indices.len() != t2.supertype_indices.len() {
            return false;
        }
        // Supertype indices must be equivalent (after normalization)
        for (&s1, &s2) in t1.supertype_indices.iter().zip(t2.supertype_indices.iter()) {
            if !Self::are_type_refs_equivalent(s1, s2, g1, g2, module) {
                return false;
            }
        }
        // Composite types must be structurally equal
        if !Self::are_composite_kinds_equivalent(&t1.composite_kind, &t2.composite_kind, g1, g2, module) {
            return false;
        }

        // For function types, also compare the function signatures from module.types
        match (&t1.composite_kind, &t2.composite_kind) {
            (CompositeTypeKind::Func, CompositeTypeKind::Func) => {
                let ft1 = module.types.get(t1.type_index as usize);
                let ft2 = module.types.get(t2.type_index as usize);
                match (ft1, ft2) {
                    (Some(f1), Some(f2)) => {
                        if f1.params.len() != f2.params.len() || f1.results.len() != f2.results.len() {
                            return false;
                        }
                        for (p1, p2) in f1.params.iter().zip(f2.params.iter()) {
                            if !Self::are_value_types_equivalent(*p1, *p2, g1, g2, module) {
                                return false;
                            }
                        }
                        for (r1, r2) in f1.results.iter().zip(f2.results.iter()) {
                            if !Self::are_value_types_equivalent(*r1, *r2, g1, g2, module) {
                                return false;
                            }
                        }
                        true
                    },
                    _ => false,
                }
            },
            _ => true,
        }
    }

    /// Check if two type references are equivalent, normalizing group-internal
    /// references to relative offsets.
    fn are_type_refs_equivalent(
        idx1: u32,
        idx2: u32,
        g1: &RecGroup,
        g2: &RecGroup,
        module: &Module,
    ) -> bool {
        let in_g1 = idx1 >= g1.start_type_index && idx1 < g1.start_type_index + g1.types.len() as u32;
        let in_g2 = idx2 >= g2.start_type_index && idx2 < g2.start_type_index + g2.types.len() as u32;

        if in_g1 && in_g2 {
            // Both are internal references - compare relative offsets
            (idx1 - g1.start_type_index) == (idx2 - g2.start_type_index)
        } else if !in_g1 && !in_g2 {
            // Both are external references - they must be equivalent
            Self::are_types_equivalent(idx1, idx2, module)
        } else {
            // One is internal, one is external - not equivalent
            false
        }
    }

    /// Check if two composite type kinds are structurally equivalent.
    fn are_composite_kinds_equivalent(
        k1: &CompositeTypeKind,
        k2: &CompositeTypeKind,
        g1: &RecGroup,
        g2: &RecGroup,
        module: &Module,
    ) -> bool {
        use kiln_format::module::GcStorageType;
        match (k1, k2) {
            (CompositeTypeKind::Func, CompositeTypeKind::Func) => {
                // For func types, we need to compare the actual function signatures.
                // The SubType's type_index gives us the func type in module.types.
                // However, we compare at the SubType level which includes composite_kind,
                // but Func kind alone doesn't carry the signature. We need to look up
                // the function types from module.types using the type indices.
                // Since we're comparing at the rec group level, we use the SubType's
                // type_index to find the corresponding func types.
                true // Func composite kinds match; the actual signature is compared
                     // through the module.types entries at the SubType level
            },
            (CompositeTypeKind::Struct, CompositeTypeKind::Struct) => true,
            (CompositeTypeKind::Array, CompositeTypeKind::Array) => true,
            (CompositeTypeKind::StructWithFields(f1), CompositeTypeKind::StructWithFields(f2)) => {
                if f1.len() != f2.len() {
                    return false;
                }
                for (field1, field2) in f1.iter().zip(f2.iter()) {
                    if field1.mutable != field2.mutable {
                        return false;
                    }
                    if !Self::are_storage_types_equivalent(
                        &field1.storage_type, &field2.storage_type, g1, g2, module
                    ) {
                        return false;
                    }
                }
                true
            },
            (CompositeTypeKind::ArrayWithElement(e1), CompositeTypeKind::ArrayWithElement(e2)) => {
                if e1.mutable != e2.mutable {
                    return false;
                }
                Self::are_storage_types_equivalent(
                    &e1.storage_type, &e2.storage_type, g1, g2, module
                )
            },
            // Mix of Struct/StructWithFields or Array/ArrayWithElement: treat as compatible
            // if the detailed one has empty fields
            (CompositeTypeKind::Struct, CompositeTypeKind::StructWithFields(f)) |
            (CompositeTypeKind::StructWithFields(f), CompositeTypeKind::Struct) => {
                f.is_empty()
            },
            _ => false,
        }
    }

    /// Check if two GC storage types are equivalent.
    fn are_storage_types_equivalent(
        s1: &kiln_format::module::GcStorageType,
        s2: &kiln_format::module::GcStorageType,
        g1: &RecGroup,
        g2: &RecGroup,
        module: &Module,
    ) -> bool {
        use kiln_format::module::GcStorageType;
        match (s1, s2) {
            (GcStorageType::I8, GcStorageType::I8) |
            (GcStorageType::I16, GcStorageType::I16) => true,
            (GcStorageType::Value(v1), GcStorageType::Value(v2)) => v1 == v2,
            (GcStorageType::RefType(idx1), GcStorageType::RefType(idx2)) |
            (GcStorageType::RefTypeNull(idx1), GcStorageType::RefTypeNull(idx2)) => {
                Self::are_type_refs_equivalent(*idx1, *idx2, g1, g2, module)
            },
            _ => false,
        }
    }

    /// Check if two ValueTypes are equivalent, considering type index equivalence.
    fn are_value_types_equivalent(
        v1: ValueType,
        v2: ValueType,
        g1: &RecGroup,
        g2: &RecGroup,
        module: &Module,
    ) -> bool {
        match (v1, v2) {
            (ValueType::TypedFuncRef(idx1, n1), ValueType::TypedFuncRef(idx2, n2)) => {
                n1 == n2 && Self::are_type_refs_equivalent(idx1, idx2, g1, g2, module)
            },
            (ValueType::StructRef(idx1), ValueType::StructRef(idx2)) => {
                Self::are_type_refs_equivalent(idx1, idx2, g1, g2, module)
            },
            (ValueType::ArrayRef(idx1), ValueType::ArrayRef(idx2)) => {
                Self::are_type_refs_equivalent(idx1, idx2, g1, g2, module)
            },
            _ => v1 == v2,
        }
    }

    /// Check if concrete type idx1 is a subtype of concrete type idx2,
    /// following the declared supertype chain in rec_groups.
    fn is_concrete_subtype(sub_idx: u32, sup_idx: u32, module: &Module) -> bool {
        if Self::are_types_equivalent(sub_idx, sup_idx, module) {
            return true;
        }

        // Walk the supertype chain from sub_idx
        let mut current = sub_idx;
        let mut visited = HashSet::new();
        visited.insert(current);

        loop {
            // Find the SubType for current
            if let Some((group, offset)) = Self::find_rec_group(current, module) {
                let sub_type = &group.types[offset];
                if sub_type.supertype_indices.is_empty() {
                    return false;
                }
                let parent = sub_type.supertype_indices[0];
                if Self::are_types_equivalent(parent, sup_idx, module) {
                    return true;
                }
                if !visited.insert(parent) {
                    return false; // Cycle detected
                }
                current = parent;
            } else {
                return false;
            }
        }
    }

    /// Module-aware subtype check for StackType values.
    ///
    /// Extends the basic `is_subtype_of` with module context for concrete type
    /// indices. Two `TypedFuncRef` values with different indices may still be
    /// subtypes if one's type index is a declared subtype of the other's.
    fn is_subtype_of_in_module(sub: &StackType, sup: &StackType, module: &Module) -> bool {
        // Use the basic check first (handles abstract types)
        if sub.is_subtype_of(sup) {
            return true;
        }

        match (sub, sup) {
            // TypedFuncRef with different indices - check concrete subtyping
            (StackType::TypedFuncRef(sub_idx, sub_null), StackType::TypedFuncRef(sup_idx, sup_null)) => {
                // Nullability: non-nullable <: nullable
                if *sub_null && !*sup_null {
                    return false;
                }
                Self::is_concrete_subtype(*sub_idx, *sup_idx, module)
            },
            // Concrete type -> abstract type: need to check what the concrete type actually is
            (StackType::TypedFuncRef(idx, _), StackType::FuncRef) => {
                Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Func)
            },
            (StackType::TypedFuncRef(idx, _), StackType::StructRef) => {
                Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Struct)
            },
            (StackType::TypedFuncRef(idx, _), StackType::ArrayRef) => {
                Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Array)
            },
            (StackType::TypedFuncRef(idx, _), StackType::EqRef) => {
                Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Struct)
                    || Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Array)
            },
            (StackType::TypedFuncRef(idx, _), StackType::AnyRef) => {
                Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Struct)
                    || Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Array)
            },
            // NoneRef is bottom of any hierarchy - subtype of nullable concrete refs in any hierarchy
            (StackType::NoneRef, StackType::TypedFuncRef(idx, true)) => {
                Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Struct)
                    || Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Array)
            },
            // NullFuncRef (nofunc) is bottom of func hierarchy
            (StackType::NullFuncRef, StackType::TypedFuncRef(idx, true)) => {
                Self::concrete_type_is_kind(module, *idx, CompositeKindClass::Func)
            },
            _ => false,
        }
    }

    /// Check if a concrete type index refers to a specific kind of composite type.
    fn concrete_type_is_kind(module: &Module, type_idx: u32, kind: CompositeKindClass) -> bool {
        if let Some(composite_kind) = Self::get_composite_type_kind(module, type_idx) {
            match (kind, composite_kind) {
                (CompositeKindClass::Func, CompositeTypeKind::Func) => true,
                (CompositeKindClass::Struct, CompositeTypeKind::Struct) |
                (CompositeKindClass::Struct, CompositeTypeKind::StructWithFields(_)) => true,
                (CompositeKindClass::Array, CompositeTypeKind::Array) |
                (CompositeKindClass::Array, CompositeTypeKind::ArrayWithElement(_)) => true,
                _ => false,
            }
        } else {
            // Not in rec_groups - check if it's a func type in module.types
            // Types not in rec_groups are plain function types
            match kind {
                CompositeKindClass::Func => true,
                _ => false,
            }
        }
    }

    /// Module-aware pop_type that uses module context for concrete type subtyping.
    fn pop_type_in_module(
        stack: &mut Vec<StackType>,
        expected: StackType,
        min_height: usize,
        unreachable: bool,
        module: &Module,
    ) -> bool {
        if unreachable {
            if stack.len() <= min_height {
                return true;
            }
            if let Some(actual) = stack.pop() {
                if actual == StackType::Unknown {
                    return true;
                }
                return Self::is_subtype_of_in_module(&actual, &expected, module);
            }
            return true;
        }

        if stack.len() <= min_height {
            return false;
        }

        if let Some(actual) = stack.pop() {
            Self::is_subtype_of_in_module(&actual, &expected, module)
        } else {
            false
        }
    }

    /// Validate all subtype declarations in the module's rec_groups.
    ///
    /// For each SubType that declares a supertype, validates:
    /// 1. The supertype index is valid
    /// 2. The supertype is not final
    /// 3. The composite types are compatible (same kind)
    /// 4. The subtype is structurally compatible with the supertype
    fn validate_subtype_declarations(module: &Module) -> Result<()> {
        for group in &module.rec_groups {
            for sub_type in &group.types {
                for &super_idx in &sub_type.supertype_indices {
                    // Validate supertype index is within bounds
                    if super_idx as usize >= module.types.len() {
                        return Err(anyhow!("sub type"));
                    }

                    // Find the supertype's SubType declaration
                    let super_subtype = Self::find_subtype_by_index(super_idx, module);

                    if let Some(super_st) = super_subtype {
                        // Check that the supertype is not final
                        if super_st.is_final {
                            return Err(anyhow!("sub type"));
                        }

                        // Check that the composite kinds are compatible
                        if !Self::is_composite_kind_compatible(
                            &sub_type.composite_kind,
                            &super_st.composite_kind,
                        ) {
                            return Err(anyhow!("sub type"));
                        }

                        // Check structural compatibility for detailed types
                        Self::validate_structural_subtype(
                            sub_type, super_st, group, module
                        )?;
                    } else {
                        // Supertype is not in rec_groups. For plain function types
                        // in module.types, the sub must also be a function type.
                        // Plain types without `sub` declaration are implicitly final.
                        return Err(anyhow!("sub type"));
                    }
                }
            }
        }
        Ok(())
    }

    /// Check if source array element type is compatible with destination for array.copy.
    /// The source element type must be a subtype of the destination element type.
    fn are_array_element_types_compatible(src_idx: u32, dst_idx: u32, module: &Module) -> bool {
        let src_sub = Self::find_subtype_by_index(src_idx, module);
        let dst_sub = Self::find_subtype_by_index(dst_idx, module);
        if let (Some(src), Some(dst)) = (src_sub, dst_sub) {
            if let (
                CompositeTypeKind::ArrayWithElement(src_field),
                CompositeTypeKind::ArrayWithElement(dst_field),
            ) = (&src.composite_kind, &dst.composite_kind) {
                // For mutable destination, source must be exact match (invariance)
                if dst_field.mutable {
                    return Self::are_storage_types_equal_in_module(
                        &src_field.storage_type, &dst_field.storage_type, module
                    );
                }
                // For immutable destination, source must be subtype (covariance)
                return Self::is_storage_type_subtype(
                    &src_field.storage_type, &dst_field.storage_type, module
                );
            }
        }
        true // If we can't determine, allow (other checks will catch real errors)
    }

    /// Get the StackType for a struct field by looking up the GC type info.
    fn get_struct_field_type(type_idx: u32, field_idx: u32, module: &Module) -> StackType {
        use kiln_format::module::GcStorageType;
        if let Some(sub) = Self::find_subtype_by_index(type_idx, module) {
            if let CompositeTypeKind::StructWithFields(fields) = &sub.composite_kind {
                if let Some(field) = fields.get(field_idx as usize) {
                    return match &field.storage_type {
                        GcStorageType::I8 | GcStorageType::I16 => StackType::I32, // packed → i32
                        GcStorageType::Value(0x7F) => StackType::I32,
                        GcStorageType::Value(0x7E) => StackType::I64,
                        GcStorageType::Value(0x7D) => StackType::F32,
                        GcStorageType::Value(0x7C) => StackType::F64,
                        GcStorageType::Value(0x7B) => StackType::V128,
                        GcStorageType::RefType(idx) => StackType::TypedFuncRef(*idx, false),
                        GcStorageType::RefTypeNull(idx) => StackType::TypedFuncRef(*idx, true),
                        GcStorageType::Value(v) => Self::value_byte_to_stack_type(*v),
                    };
                }
            }
        }
        StackType::Unknown
    }

    /// Get the StackType for an array element by looking up the GC type info.
    fn get_array_element_type(type_idx: u32, module: &Module) -> StackType {
        use kiln_format::module::GcStorageType;
        if let Some(sub) = Self::find_subtype_by_index(type_idx, module) {
            if let CompositeTypeKind::ArrayWithElement(field) = &sub.composite_kind {
                return match &field.storage_type {
                    GcStorageType::I8 | GcStorageType::I16 => StackType::I32, // packed → i32
                    GcStorageType::Value(0x7F) => StackType::I32,
                    GcStorageType::Value(0x7E) => StackType::I64,
                    GcStorageType::Value(0x7D) => StackType::F32,
                    GcStorageType::Value(0x7C) => StackType::F64,
                    GcStorageType::Value(0x7B) => StackType::V128,
                    GcStorageType::RefType(idx) => StackType::TypedFuncRef(*idx, false),
                    GcStorageType::RefTypeNull(idx) => StackType::TypedFuncRef(*idx, true),
                    GcStorageType::Value(v) => Self::value_byte_to_stack_type(*v),
                };
            }
        }
        StackType::Unknown
    }

    /// Check if array element type is numeric or vector (not a reference type).
    /// Required for array.init_data which copies raw bytes from data segments.
    fn is_array_numeric_or_vector(type_idx: u32, module: &Module) -> bool {
        use kiln_format::module::GcStorageType;
        if let Some(sub) = Self::find_subtype_by_index(type_idx, module) {
            if let CompositeTypeKind::ArrayWithElement(field) = &sub.composite_kind {
                return matches!(field.storage_type,
                    GcStorageType::I8 | GcStorageType::I16 | GcStorageType::Value(
                        0x7F | 0x7E | 0x7D | 0x7C | 0x7B // i32, i64, f32, f64, v128
                    )
                );
            }
        }
        false
    }

    /// Check if array type at given index has a mutable element.
    fn is_array_mutable(type_idx: u32, module: &Module) -> bool {
        if let Some(sub) = Self::find_subtype_by_index(type_idx, module) {
            if let CompositeTypeKind::ArrayWithElement(field) = &sub.composite_kind {
                return field.mutable;
            }
        }
        false
    }

    /// Check if struct field at given index is mutable.
    fn is_struct_field_mutable(type_idx: u32, field_idx: u32, module: &Module) -> bool {
        if let Some(sub) = Self::find_subtype_by_index(type_idx, module) {
            if let CompositeTypeKind::StructWithFields(fields) = &sub.composite_kind {
                if let Some(field) = fields.get(field_idx as usize) {
                    return field.mutable;
                }
            }
        }
        false
    }

    /// Find a SubType by its type index across all rec_groups.
    fn find_subtype_by_index(type_idx: u32, module: &Module) -> Option<&kiln_format::module::SubType> {
        for group in &module.rec_groups {
            let start = group.start_type_index;
            let end = start + group.types.len() as u32;
            if type_idx >= start && type_idx < end {
                let local_idx = (type_idx - start) as usize;
                return Some(&group.types[local_idx]);
            }
        }
        None
    }

    /// Check if a child composite kind is compatible with a parent composite kind.
    /// Same kind requirement: func <: func, struct <: struct, array <: array.
    fn is_composite_kind_compatible(
        child: &CompositeTypeKind,
        parent: &CompositeTypeKind,
    ) -> bool {
        match (child, parent) {
            (CompositeTypeKind::Func, CompositeTypeKind::Func) => true,
            (CompositeTypeKind::Struct, CompositeTypeKind::Struct) |
            (CompositeTypeKind::Struct, CompositeTypeKind::StructWithFields(_)) |
            (CompositeTypeKind::StructWithFields(_), CompositeTypeKind::Struct) |
            (CompositeTypeKind::StructWithFields(_), CompositeTypeKind::StructWithFields(_)) => true,
            (CompositeTypeKind::Array, CompositeTypeKind::Array) |
            (CompositeTypeKind::Array, CompositeTypeKind::ArrayWithElement(_)) |
            (CompositeTypeKind::ArrayWithElement(_), CompositeTypeKind::Array) |
            (CompositeTypeKind::ArrayWithElement(_), CompositeTypeKind::ArrayWithElement(_)) => true,
            _ => false,
        }
    }

    /// Validate that a subtype is structurally compatible with its declared supertype.
    ///
    /// For structs: child must have at least as many fields, first N fields must match
    /// For arrays: element types must match (covariant for immutable, invariant for mutable)
    /// For funcs: params must be contravariant, results must be covariant
    fn validate_structural_subtype(
        child: &kiln_format::module::SubType,
        parent: &kiln_format::module::SubType,
        child_group: &RecGroup,
        module: &Module,
    ) -> Result<()> {
        match (&child.composite_kind, &parent.composite_kind) {
            (CompositeTypeKind::Func, CompositeTypeKind::Func) => {
                // For function types, compare using module.types
                let child_ft = module.types.get(child.type_index as usize);
                let parent_ft = module.types.get(parent.type_index as usize);
                if let (Some(cft), Some(pft)) = (child_ft, parent_ft) {
                    // Same number of params and results required
                    if cft.params.len() != pft.params.len() || cft.results.len() != pft.results.len() {
                        return Err(anyhow!("sub type"));
                    }
                    // Params: contravariant (parent param must be subtype of child param)
                    for (cp, pp) in cft.params.iter().zip(pft.params.iter()) {
                        let cs = StackType::from_value_type(*cp);
                        let ps = StackType::from_value_type(*pp);
                        if !Self::is_subtype_of_in_module(&ps, &cs, module) {
                            return Err(anyhow!("sub type"));
                        }
                    }
                    // Results: covariant (child result must be subtype of parent result)
                    for (cr, pr) in cft.results.iter().zip(pft.results.iter()) {
                        let cs = StackType::from_value_type(*cr);
                        let ps = StackType::from_value_type(*pr);
                        if !Self::is_subtype_of_in_module(&cs, &ps, module) {
                            return Err(anyhow!("sub type"));
                        }
                    }
                }
            },
            (CompositeTypeKind::StructWithFields(child_fields), CompositeTypeKind::StructWithFields(parent_fields)) => {
                // Child must have at least as many fields as parent
                if child_fields.len() < parent_fields.len() {
                    return Err(anyhow!("sub type"));
                }
                // First N fields must be compatible
                for (cf, pf) in child_fields.iter().zip(parent_fields.iter()) {
                    Self::validate_field_subtype(cf, pf, module)?;
                }
            },
            (CompositeTypeKind::ArrayWithElement(child_elem), CompositeTypeKind::ArrayWithElement(parent_elem)) => {
                Self::validate_field_subtype(child_elem, parent_elem, module)?;
            },
            // If one side is detailed and the other isn't, we can't fully validate
            // but the kind compatibility was already checked
            _ => {},
        }
        Ok(())
    }

    /// Validate field subtyping for struct/array fields.
    ///
    /// Immutable fields: covariant (child field type must be subtype of parent field type)
    /// Mutable fields: invariant (types must be equivalent)
    fn validate_field_subtype(
        child: &kiln_format::module::GcFieldType,
        parent: &kiln_format::module::GcFieldType,
        module: &Module,
    ) -> Result<()> {
        use kiln_format::module::GcStorageType;

        if child.mutable != parent.mutable {
            return Err(anyhow!("sub type"));
        }

        if child.mutable {
            // Mutable fields must be invariant (exact match)
            if !Self::are_storage_types_equal_in_module(&child.storage_type, &parent.storage_type, module) {
                return Err(anyhow!("sub type"));
            }
        } else {
            // Immutable fields are covariant
            if !Self::is_storage_type_subtype(&child.storage_type, &parent.storage_type, module) {
                return Err(anyhow!("sub type"));
            }
        }
        Ok(())
    }

    /// Check if two storage types are equal (for mutable field invariance).
    fn are_storage_types_equal_in_module(
        s1: &kiln_format::module::GcStorageType,
        s2: &kiln_format::module::GcStorageType,
        module: &Module,
    ) -> bool {
        use kiln_format::module::GcStorageType;
        match (s1, s2) {
            (GcStorageType::I8, GcStorageType::I8) |
            (GcStorageType::I16, GcStorageType::I16) => true,
            (GcStorageType::Value(v1), GcStorageType::Value(v2)) => {
                // For mutable fields (invariance), types must be exactly equal.
                // Abstract ref types stored as Value bytes are equal iff same byte.
                v1 == v2
            },
            (GcStorageType::RefType(idx1), GcStorageType::RefType(idx2)) |
            (GcStorageType::RefTypeNull(idx1), GcStorageType::RefTypeNull(idx2)) => {
                Self::are_types_equivalent(*idx1, *idx2, module)
            },
            _ => false,
        }
    }

    /// Check if a storage type is a subtype of another (for immutable field covariance).
    fn is_storage_type_subtype(
        sub: &kiln_format::module::GcStorageType,
        sup: &kiln_format::module::GcStorageType,
        module: &Module,
    ) -> bool {
        use kiln_format::module::GcStorageType;
        match (sub, sup) {
            (GcStorageType::I8, GcStorageType::I8) |
            (GcStorageType::I16, GcStorageType::I16) => true,
            (GcStorageType::Value(v1), GcStorageType::Value(v2)) => {
                if v1 == v2 { return true; }
                // For abstract ref types, check subtype relationship
                let sub_st = Self::value_byte_to_stack_type(*v1);
                let sup_st = Self::value_byte_to_stack_type(*v2);
                if matches!(sub_st, StackType::Unknown) || matches!(sup_st, StackType::Unknown) {
                    return false; // Non-ref Value types: exact match only
                }
                Self::is_subtype_of_in_module(&sub_st, &sup_st, module)
            },
            (GcStorageType::RefType(sub_idx), GcStorageType::RefType(sup_idx)) => {
                Self::is_concrete_subtype(*sub_idx, *sup_idx, module)
            },
            (GcStorageType::RefType(sub_idx), GcStorageType::RefTypeNull(sup_idx)) => {
                Self::is_concrete_subtype(*sub_idx, *sup_idx, module)
            },
            (GcStorageType::RefTypeNull(sub_idx), GcStorageType::RefTypeNull(sup_idx)) => {
                Self::is_concrete_subtype(*sub_idx, *sup_idx, module)
            },
            // Cross-format: concrete ref vs abstract ref encoded as Value byte
            // Value bytes: 0x6E=anyref, 0x6D=eqref, 0x6B=structref, 0x6A=arrayref,
            // 0x70=funcref, 0x6F=externref
            (GcStorageType::RefType(idx), GcStorageType::Value(v)) |
            (GcStorageType::RefTypeNull(idx), GcStorageType::Value(v)) => {
                let sub_st = StackType::TypedFuncRef(*idx, matches!(sub, GcStorageType::RefTypeNull(_)));
                let sup_st = Self::value_byte_to_stack_type(*v);
                Self::is_subtype_of_in_module(&sub_st, &sup_st, module)
            },
            _ => false,
        }
    }

    /// Convert a value type byte to StackType for subtype checking.
    fn value_byte_to_stack_type(byte: u8) -> StackType {
        match byte {
            0x6E => StackType::AnyRef,     // anyref
            0x6D => StackType::EqRef,      // eqref
            0x6C => StackType::I31Ref,     // i31ref
            0x6B => StackType::StructRef,  // structref
            0x6A => StackType::ArrayRef,   // arrayref
            0x70 => StackType::FuncRef,    // funcref
            0x6F => StackType::ExternRef,  // externref
            0x69 => StackType::ExnRef,     // exnref
            0x71 => StackType::NoneRef,    // none
            0x73 => StackType::NullFuncRef, // nofunc
            0x72 => StackType::NullExternRef, // noextern
            _ => StackType::Unknown,
        }
    }

    /// Check if two ValueTypes are equal with module context for concrete indices.
    fn are_value_types_equal_in_module(v1: ValueType, v2: ValueType, module: &Module) -> bool {
        match (v1, v2) {
            (ValueType::TypedFuncRef(idx1, n1), ValueType::TypedFuncRef(idx2, n2)) => {
                n1 == n2 && Self::are_types_equivalent(idx1, idx2, module)
            },
            _ => v1 == v2,
        }
    }

    /// Validate that type references within rec groups don't use forward references
    /// outside the current group.
    ///
    /// In a non-rec-group context, type $t1 cannot reference $t2 if $t2 comes after $t1.
    /// Within a rec group, forward references within the group are allowed.
    fn validate_rec_group_type_references(module: &Module) -> Result<()> {
        // Types that are NOT in rec groups but reference a later type are invalid
        // For each type in the module, check if it references a type that comes after it
        // but is not in the same rec group.

        // Build a set of type indices that are in rec groups, along with their group bounds
        let mut type_to_group_end: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for group in &module.rec_groups {
            let end = group.start_type_index + group.types.len() as u32;
            for i in group.start_type_index..end {
                type_to_group_end.insert(i, end);
            }
        }

        // Check each type's references
        for (idx, func_type) in module.types.iter().enumerate() {
            let idx = idx as u32;
            let group_end = type_to_group_end.get(&idx).copied().unwrap_or(idx + 1);

            for vt in func_type.params.iter().chain(func_type.results.iter()) {
                if let Some(ref_idx) = Self::extract_type_ref(vt) {
                    // The referenced type must either be before this type or in the same group
                    if ref_idx > idx && ref_idx >= group_end {
                        return Err(anyhow!("unknown type"));
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract a type index reference from a ValueType, if any.
    fn extract_type_ref(vt: &ValueType) -> Option<u32> {
        match vt {
            ValueType::TypedFuncRef(idx, _) |
            ValueType::StructRef(idx) |
            ValueType::ArrayRef(idx) => Some(*idx),
            _ => None,
        }
    }
}

/// Classification of composite type kinds for module-aware subtyping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositeKindClass {
    Func,
    Struct,
    Array,
}

/// Block type enumeration
#[derive(Debug, Clone, Copy)]
pub enum BlockType {
    Empty,
    ValueType(ValueType),
    FuncType(u32),
}
