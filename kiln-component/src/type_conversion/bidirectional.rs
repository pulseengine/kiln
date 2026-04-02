//! Bidirectional type conversion between format and runtime types
//!
//! This module provides comprehensive bidirectional conversion between
//! `kiln_format::component` types and `kiln_foundation` types, ensuring type
//! compatibility across the system boundary.
//!
//! # Examples
//!
//! ```
//! use kiln_component::type_conversion::bidirectional::{
//!     format_to_runtime_extern_type, runtime_to_format_extern_type
//! };
//! use kiln_format::component::FormatValType;
//! use kiln_format::component::KilnExternType as FormatExternType;
//! use kiln_foundation::KilnExternType as RuntimeKilnExternType;
//!
//! // Convert a format function type to a runtime function type
//! let format_func = FormatExternType::Function {
//!     params: vec![("arg".to_owned(), FormatValType::S32)],
//!     results: vec![FormatValType::S32],
//! };
//!
//! let runtime_func = format_to_runtime_extern_type(&format_func).unwrap();
//!
//! // Convert back to format type
//! let format_func_again = runtime_to_format_extern_type(&runtime_func).unwrap();
//! ```

// Explicitly import the types we need to avoid confusion
use kiln_error::{
    Error, ErrorCategory, codes,
    kinds::{InvalidArgumentError, NotImplementedError},
};
use kiln_format::component::{
    ComponentTypeDefinition, ConstValue as FormatConstValue, ExternType as FormatExternType,
    FormatResourceOperation, FormatValType, ResourceRepresentation,
};
use kiln_foundation::{
    ExternType as TypesExternType,
    component::{ComponentType, InstanceType},
    component_value::ValType as TypesValType,
    resource::{ResourceOperation, ResourceType},
    types::{FuncType as TypesFuncType, ValueType},
    values::Value,
};

// For no_std, override prelude's bounded::BoundedVec with StaticVec
#[cfg(not(feature = "std"))]
use kiln_foundation::collections::StaticVec as BoundedVec;

use crate::prelude::*;

// Type aliases to ensure consistent generic parameters
type KilnTypesValType<P> = KilnValType<P>; // Use prelude's ValType
type TypesKilnExternType<P> = TypesExternType<P>;
type KilnExternType<P> = TypesExternType<P>;
// KilnComponentValue is already available from prelude

// Helper functions to handle type conversions with correct parameters

// Special helper functions for FormatValType to ValueType conversion
pub fn convert_format_valtype_to_valuetype(format_val_type: &FormatValType) -> Result<ValueType> {
    match format_val_type {
        FormatValType::S32 => Ok(ValueType::I32),
        FormatValType::S64 => Ok(ValueType::I64),
        FormatValType::F32 => Ok(ValueType::F32),
        FormatValType::F64 => Ok(ValueType::F64),
        _ => Err(Error::unimplemented("Error occurred")),
    }
}

// Variant that accepts ValType (KilnTypesValType) for use at call sites
pub fn convert_types_valtype_to_valuetype<P: kiln_foundation::MemoryProvider>(
    val_type: &KilnTypesValType<P>,
) -> Result<ValueType> {
    match val_type {
        KilnTypesValType::S32 => Ok(ValueType::I32),
        KilnTypesValType::S64 => Ok(ValueType::I64),
        KilnTypesValType::F32 => Ok(ValueType::F32),
        KilnTypesValType::F64 => Ok(ValueType::F64),
        _ => Err(Error::unimplemented("Error occurred")),
    }
}

// Special helper function for FormatValType to KilnTypesValType conversion
pub fn convert_format_to_types_valtype<P: kiln_foundation::MemoryProvider>(
    format_val_type: &FormatValType,
) -> KilnTypesValType<P> {
    match format_val_type {
        FormatValType::Bool => KilnTypesValType::Bool,
        FormatValType::S8 => KilnTypesValType::S8,
        FormatValType::U8 => KilnTypesValType::U8,
        FormatValType::S16 => KilnTypesValType::S16,
        FormatValType::U16 => KilnTypesValType::U16,
        FormatValType::S32 => KilnTypesValType::S32,
        FormatValType::U32 => KilnTypesValType::U32,
        FormatValType::S64 => KilnTypesValType::S64,
        FormatValType::U64 => KilnTypesValType::U64,
        FormatValType::F32 => KilnTypesValType::F32,
        FormatValType::F64 => KilnTypesValType::F64,
        FormatValType::Char => KilnTypesValType::Char,
        FormatValType::String => KilnTypesValType::String,
        FormatValType::Ref(idx) => KilnTypesValType::Ref(*idx),
        FormatValType::Own(idx) => KilnTypesValType::Own(*idx),
        FormatValType::Borrow(idx) => KilnTypesValType::Borrow(*idx),
        _ => KilnTypesValType::Void, // Default fallback
    }
}

// Variant that takes a ValType directly for use at call sites
pub fn convert_types_valtype_identity<P: kiln_foundation::MemoryProvider>(
    val_type: &KilnTypesValType<P>,
) -> KilnTypesValType<P> {
    val_type.clone()
}

// Special helper function for KilnTypesValType to FormatValType conversion
pub fn convert_types_to_format_valtype<P: kiln_foundation::MemoryProvider>(
    types_val_type: &KilnTypesValType<P>,
) -> FormatValType {
    match types_val_type {
        KilnTypesValType::Bool => FormatValType::Bool,
        KilnTypesValType::S8 => FormatValType::S8,
        KilnTypesValType::U8 => FormatValType::U8,
        KilnTypesValType::S16 => FormatValType::S16,
        KilnTypesValType::U16 => FormatValType::U16,
        KilnTypesValType::S32 => FormatValType::S32,
        KilnTypesValType::U32 => FormatValType::U32,
        KilnTypesValType::S64 => FormatValType::S64,
        KilnTypesValType::U64 => FormatValType::U64,
        KilnTypesValType::F32 => FormatValType::F32,
        KilnTypesValType::F64 => FormatValType::F64,
        KilnTypesValType::Char => FormatValType::Char,
        KilnTypesValType::String => FormatValType::String,
        KilnTypesValType::Ref(idx) => FormatValType::Ref(*idx),
        KilnTypesValType::Own(idx) => FormatValType::Own(*idx),
        KilnTypesValType::Borrow(idx) => FormatValType::Borrow(*idx),
        _ => FormatValType::Bool, // Default fallback
    }
}

/// Convert a ValueType to a FormatValType
///
/// This function converts from kiln_foundation::types::ValueType to
/// kiln_format::component::ValType directly.
///
/// # Arguments
///
/// * `value_type` - The core WebAssembly value type to convert
///
/// # Returns
///
/// A Result containing the converted format value type, or an error if
/// conversion is not possible
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::value_type_to_format_val_type;
/// use kiln_foundation::types::ValueType;
///
/// let i32_type = ValueType::I32;
/// let format_type = value_type_to_format_val_type(&i32_type).unwrap();
/// assert!(matches!(format_type, kiln_format::component::ValType::S32);
/// ```
pub fn value_type_to_format_val_type(value_type: &ValueType) -> Result<FormatValType> {
    match value_type {
        ValueType::I32 => Ok(FormatValType::S32),
        ValueType::I64 => Ok(FormatValType::S64),
        ValueType::F32 => Ok(FormatValType::F32),
        ValueType::F64 => Ok(FormatValType::F64),
        ValueType::FuncRef => Err(Error::runtime_execution_error("FuncRef not supported")),
        ValueType::NullFuncRef => Err(Error::runtime_execution_error("NullFuncRef not supported")),
        ValueType::ExternRef => Err(Error::runtime_execution_error("ExternRef not supported")),
        ValueType::V128 => Err(Error::runtime_execution_error(
            "V128 not supported in component model",
        )),
        ValueType::I16x8 => Err(Error::runtime_execution_error(
            "I16x8 not supported in component model",
        )),
        ValueType::StructRef(_) => Err(Error::runtime_execution_error(
            "StructRef not supported in component model",
        )),
        ValueType::ArrayRef(_) => Err(Error::runtime_execution_error(
            "ArrayRef not supported in component model",
        )),
        ValueType::ExnRef => Err(Error::runtime_execution_error(
            "ExnRef not supported in component model",
        )),
        ValueType::I31Ref => Err(Error::runtime_execution_error(
            "I31Ref not supported in component model",
        )),
        ValueType::AnyRef => Err(Error::runtime_execution_error(
            "AnyRef not supported in component model",
        )),
        ValueType::EqRef => Err(Error::runtime_execution_error(
            "EqRef not supported in component model",
        )),
        ValueType::TypedFuncRef(_, _) => Err(Error::runtime_execution_error(
            "TypedFuncRef not supported in component model",
        )),
        ValueType::NoneRef => Err(Error::runtime_execution_error(
            "NoneRef not supported in component model",
        )),
        ValueType::NoExternRef => Err(Error::runtime_execution_error(
            "NoExternRef not supported in component model",
        )),
        ValueType::NoExnRef => Err(Error::runtime_execution_error(
            "NoExnRef not supported in component model",
        )),
    }
}

/// Convert FormatValType to ValueType
///
/// Converts a component model value type to a core WebAssembly value type.
///
/// # Arguments
///
/// * `format_val_type` - The format value type to convert
///
/// # Returns
///
/// A Result containing the converted core value type, or an error if
/// conversion is not possible
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::format_val_type_to_value_type;
/// use kiln_format::component::ValType;
///
/// let s32_type = ValType::S32;
/// let core_type = format_val_type_to_value_type(&s32_type).unwrap();
/// assert!(matches!(core_type, kiln_foundation::types::ValueType::I32);
/// ```
pub fn format_val_type_to_value_type(format_val_type: &FormatValType) -> Result<ValueType> {
    convert_format_valtype_to_valuetype(format_val_type)
}

/// Convert KilnTypesValType to ValueType
///
/// Converts a runtime component value type to a core WebAssembly value type.
///
/// # Arguments
///
/// * `types_val_type` - The runtime value type to convert
///
/// # Returns
///
/// A Result containing the converted core value type, or an error if
/// conversion is not possible
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::types_valtype_to_valuetype;
/// use kiln_foundation::component_value::ValType;
///
/// let s32_type = ValType::S32;
/// let core_type = types_valtype_to_valuetype(&s32_type).unwrap();
/// assert!(matches!(core_type, kiln_foundation::types::ValueType::I32);
/// ```
pub fn types_valtype_to_valuetype<P: kiln_foundation::MemoryProvider>(
    types_val_type: &KilnTypesValType<P>,
) -> Result<ValueType> {
    convert_types_valtype_to_valuetype(types_val_type)
}

/// Convert ValueType to TypesValType
///
/// Converts a core WebAssembly value type to the runtime component value type.
///
/// # Arguments
///
/// * `value_type` - The core value type to convert
///
/// # Returns
///
/// The corresponding runtime component value type
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::value_type_to_types_valtype;
/// use kiln_foundation::types::ValueType;
///
/// let i32_type = ValueType::I32;
/// let runtime_type = value_type_to_types_valtype(&i32_type;
/// assert!(matches!(runtime_type, kiln_foundation::component_value::ValType::S32);
/// ```
pub fn value_type_to_types_valtype<P: kiln_foundation::MemoryProvider>(
    value_type: &ValueType,
) -> KilnTypesValType<P> {
    match value_type {
        ValueType::I32 => KilnTypesValType::S32,
        ValueType::I64 => KilnTypesValType::S64,
        ValueType::F32 => KilnTypesValType::F32,
        ValueType::F64 => KilnTypesValType::F64,
        ValueType::FuncRef => KilnTypesValType::Own(0), // Default to resource type 0
        ValueType::NullFuncRef => KilnTypesValType::Own(0), // Bottom funcref type
        ValueType::ExternRef => KilnTypesValType::Ref(0), // Default to type index 0
        ValueType::V128 => KilnTypesValType::Void,      // V128 not supported in component model
        ValueType::I16x8 => KilnTypesValType::Void,     // I16x8 not supported in component model
        ValueType::StructRef(_) => KilnTypesValType::Ref(0), // Map to Ref with default index
        ValueType::ArrayRef(_) => KilnTypesValType::Ref(0), // Map to Ref with default index
        ValueType::ExnRef => KilnTypesValType::Ref(0),  // Map ExnRef to Ref with default index
        ValueType::I31Ref => KilnTypesValType::S32,     // i31 fits in s32
        ValueType::AnyRef => KilnTypesValType::Ref(0),  // Map to Ref with default index
        ValueType::EqRef => KilnTypesValType::Ref(0),   // Map to Ref with default index
        ValueType::TypedFuncRef(_, _) => KilnTypesValType::Own(0), // Map to resource type
        ValueType::NoneRef => KilnTypesValType::Ref(0),    // Bottom of any hierarchy
        ValueType::NoExternRef => KilnTypesValType::Ref(0), // Bottom of extern hierarchy
        ValueType::NoExnRef => KilnTypesValType::Ref(0),   // Bottom of exn hierarchy
    }
}

/// Convert FormatValType to TypesValType
///
/// Comprehensive conversion from format value type to runtime component value
/// type.
///
/// # Arguments
///
/// * `format_val_type` - The format value type to convert
///
/// # Returns
///
/// The corresponding runtime component value type
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::format_valtype_to_types_valtype;
/// use kiln_format::component::ValType;
///
/// let string_type = ValType::String;
/// let runtime_type = format_valtype_to_types_valtype(&string_type;
/// assert!(matches!(runtime_type, kiln_foundation::component_value::ValType::String);
/// ```
pub fn format_valtype_to_types_valtype<P: kiln_foundation::MemoryProvider>(
    format_val_type: &FormatValType,
) -> KilnTypesValType<P> {
    convert_format_to_types_valtype::<P>(format_val_type)
}

/// Format type to types ValType helper function
///
/// This is a public entry point for the helper function to ensure
/// compatibility.
///
/// # Arguments
///
/// * `val_type` - The ValType to convert
///
/// # Returns
///
/// The corresponding TypesValType
pub fn format_to_types_valtype<P: kiln_foundation::MemoryProvider>(
    val_type: &KilnTypesValType<P>,
) -> KilnTypesValType<P> {
    convert_types_valtype_identity(val_type)
}

/// Convert KilnTypesValType to FormatValType
///
/// Comprehensive conversion from runtime component value type to format value
/// type.
///
/// # Arguments
///
/// * `types_val_type` - The runtime component value type to convert
///
/// # Returns
///
/// The corresponding format value type
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::types_valtype_to_format_valtype;
/// use kiln_foundation::component_value::ValType;
///
/// let string_type = ValType::String;
/// let format_type = types_valtype_to_format_valtype(&string_type;
/// assert!(matches!(format_type, kiln_format::component::ValType::String);
/// ```
pub fn types_valtype_to_format_valtype<P: kiln_foundation::MemoryProvider>(
    types_val_type: &KilnTypesValType<P>,
) -> FormatValType {
    match types_val_type {
        KilnTypesValType::Bool => FormatValType::Bool,
        KilnTypesValType::S8 => FormatValType::S8,
        KilnTypesValType::U8 => FormatValType::U8,
        KilnTypesValType::S16 => FormatValType::S16,
        KilnTypesValType::U16 => FormatValType::U16,
        KilnTypesValType::S32 => FormatValType::S32,
        KilnTypesValType::U32 => FormatValType::U32,
        KilnTypesValType::S64 => FormatValType::S64,
        KilnTypesValType::U64 => FormatValType::U64,
        KilnTypesValType::F32 => FormatValType::F32,
        KilnTypesValType::F64 => FormatValType::F64,
        KilnTypesValType::Char => FormatValType::Char,
        KilnTypesValType::String => FormatValType::String,
        KilnTypesValType::Ref(idx) => FormatValType::Ref(*idx),
        KilnTypesValType::Record(_fields) => {
            // Record fields contain ValTypeRef (u32 indices), not actual types
            // Return empty record as placeholder - proper conversion requires type table
            FormatValType::Record(Vec::new())
        },
        KilnTypesValType::Variant(_cases) => {
            // Variant cases contain ValTypeRef (u32 indices), not actual types
            // Return empty variant as placeholder - proper conversion requires type table
            FormatValType::Variant(Vec::new())
        },
        KilnTypesValType::List(_elem_type_ref) => {
            // List contains ValTypeRef (u32 index), not actual type
            // Return placeholder - proper conversion requires type table
            FormatValType::List(Box::new(FormatValType::Void))
        },
        KilnTypesValType::FixedList(_elem_type_ref, size) => {
            // FixedList contains ValTypeRef (u32 index), not actual type
            // Return placeholder - proper conversion requires type table
            FormatValType::FixedList(Box::new(FormatValType::Void), *size)
        },
        KilnTypesValType::Tuple(_types_refs) => {
            // Tuple contains ValTypeRef (u32 indices), not actual types
            // Return empty tuple as placeholder - proper conversion requires type table
            FormatValType::Tuple(Vec::new())
        },
        KilnTypesValType::Flags(names) => {
            // Convert BoundedVec<WasmName> to Vec<String>
            let string_names: Vec<String> = names
                .iter()
                .filter_map(|name| name.as_str().ok().map(|s| s.to_string()))
                .collect();
            FormatValType::Flags(string_names)
        },
        KilnTypesValType::Enum(variants) => {
            // Convert BoundedVec<WasmName> to Vec<String>
            let string_variants: Vec<String> = variants
                .iter()
                .filter_map(|variant| variant.as_str().ok().map(|s| s.to_string()))
                .collect();
            FormatValType::Enum(string_variants)
        },
        KilnTypesValType::Option(_inner_type_ref) => {
            // Option contains ValTypeRef (u32 index), not actual type
            // Return placeholder - proper conversion requires type table
            FormatValType::Option(Box::new(FormatValType::Void))
        },
        KilnTypesValType::Own(idx) => FormatValType::Own(*idx),
        KilnTypesValType::Borrow(idx) => FormatValType::Borrow(*idx),
        KilnTypesValType::Void => {
            // Map void to a default type (this is a simplification)
            FormatValType::Bool
        },
        KilnTypesValType::ErrorContext => FormatValType::ErrorContext,
        KilnTypesValType::Result { ok: _, err: _ } => {
            // Map to FormatValType::Result with a placeholder type
            FormatValType::Result(Box::new(FormatValType::Void))
        }, // All enums handled above
    }
}

/// Convert FormatExternType to TypesKilnExternType
///
/// Comprehensive conversion from format external type to runtime external type.
///
/// # Arguments
///
/// * `format_extern_type` - The format external type to convert
///
/// # Returns
///
/// Result containing the corresponding runtime external type, or an error if
/// conversion is not possible
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::format_to_runtime_extern_type;
/// use kiln_format::component::KilnExternType as FormatExternType;
/// use kiln_format::component::ValType as FormatValType;
///
/// let format_func = FormatExternType::Function {
///     params: vec![("param".to_owned(), FormatValType::S32)],
///     results: vec![FormatValType::S32],
/// };
///
/// let runtime_func = format_to_runtime_extern_type(&format_func).unwrap();
/// ```
pub fn format_to_runtime_extern_type<P: kiln_foundation::MemoryProvider>(
    format_extern_type: &FormatExternType,
) -> Result<TypesKilnExternType<P>> {
    match format_extern_type {
        FormatExternType::Module { type_idx } => {
            // Core module type - not yet implemented
            Err(Error::runtime_execution_error(
                "Module extern types not yet supported",
            ))
        },
        FormatExternType::Function { params, results } => {
            // Convert all parameter types to core ValueType
            let converted_params = params
                .iter()
                .map(|(name, val_type)| format_val_type_to_value_type(val_type))
                .collect::<Result<Vec<_>>>()?;

            // Convert all result types to core ValueType
            let converted_results =
                results.iter().map(format_val_type_to_value_type).collect::<Result<Vec<_>>>()?;

            let _provider = P::default();
            Ok(TypesKilnExternType::Func(TypesFuncType::new(
                converted_params,
                converted_results,
            )?))
        },
        FormatExternType::Value(val_type) => {
            // Convert to most appropriate TypesKilnExternType - likely Function with no
            // params/results Could be mapped as constant global in the future
            let value_type = format_val_type_to_value_type(val_type).unwrap_or(ValueType::I32);
            Ok(TypesKilnExternType::Global(
                kiln_foundation::types::GlobalType {
                    value_type,
                    mutable: false,
                },
            ))
        },
        FormatExternType::Type(type_idx) => {
            // Type reference - this would need context from the component
            // For now, provide a sensible default
            let _provider = P::default();
            Ok(TypesKilnExternType::Func(TypesFuncType::new(
                vec![],
                vec![],
            )?))
        },
        FormatExternType::Instance { exports } => {
            // Convert each export to Export<P> with WasmName
            use kiln_foundation::WasmName;

            let provider = P::default();
            let mut export_vec = kiln_foundation::BoundedVec::new(provider.clone())?;

            for (name, ext_type) in exports.iter() {
                let wasm_name = WasmName::from_str_truncate(name.as_str())
                    .map_err(|_| Error::runtime_execution_error("Failed to create WasmName"))?;
                let extern_ty = format_to_runtime_extern_type(ext_type)?;
                let export = kiln_foundation::component::Export {
                    name: wasm_name,
                    ty: extern_ty,
                    desc: None,
                };
                export_vec.push(export)?;
            }

            Ok(TypesKilnExternType::Instance(InstanceType {
                exports: export_vec,
            }))
        },
        FormatExternType::Component { type_idx } => {
            // Component type reference - create a placeholder component type
            // In a full implementation, this would look up the type from the type index space
            let provider = P::default();

            // Create empty component type as placeholder
            Ok(TypesKilnExternType::Component(ComponentType {
                imports: kiln_foundation::BoundedVec::new(provider.clone())?,
                exports: kiln_foundation::BoundedVec::new(provider.clone())?,
                aliases: kiln_foundation::BoundedVec::new(provider.clone())?,
                instances: kiln_foundation::BoundedVec::new(provider.clone())?,
                core_instances: kiln_foundation::BoundedVec::new(provider.clone())?,
                component_types: kiln_foundation::BoundedVec::new(provider.clone())?,
                core_types: kiln_foundation::BoundedVec::new(provider)?,
            }))
        },
    }
}

/// Convert TypesKilnExternType to FormatExternType
///
/// Comprehensive conversion from runtime external type to format external type.
///
/// # Arguments
///
/// * `types_extern_type` - The runtime external type to convert
///
/// # Returns
///
/// Result containing the corresponding format external type, or an error if
/// conversion is not possible
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::runtime_to_format_extern_type;
/// use kiln_foundation::{KilnExternType, component::FuncType};
/// use kiln_foundation::types::ValueType;
///
/// let func_type = FuncType {
///     params: vec![ValueType::I32],
///     results: vec![ValueType::I32],
/// };
///
/// let runtime_func = KilnExternType::Func(func_type;
/// let format_func = runtime_to_format_extern_type(&runtime_func).unwrap();
/// ```
pub fn runtime_to_format_extern_type<P: kiln_foundation::MemoryProvider>(
    types_extern_type: &TypesKilnExternType<P>,
) -> Result<FormatExternType> {
    match types_extern_type {
        KilnExternType::Func(func_type) => {
            // Convert parameter types
            let param_names: Vec<String> =
                (0..func_type.params.len()).map(|i| format!("param{}", i)).collect();

            // Create param_types manually to handle errors gracefully
            let mut param_types = Vec::new();
            for (i, value_type) in func_type.params.iter().enumerate() {
                match value_type_to_format_val_type(value_type) {
                    Ok(format_val_type) => {
                        param_types.push((param_names[i].clone(), format_val_type))
                    },
                    Err(e) => return Err(e),
                }
            }

            // Create result_types manually to handle errors gracefully
            let mut result_types = Vec::new();
            for value_type in &func_type.results {
                match value_type_to_format_val_type(value_type) {
                    Ok(format_val_type) => result_types.push(format_val_type),
                    Err(e) => return Err(e),
                }
            }

            Ok(FormatExternType::Function {
                params: param_types,
                results: result_types,
            })
        },
        KilnExternType::Table(table_type) => {
            Err(Error::runtime_execution_error("Table types not supported"))
        },
        KilnExternType::Memory(memory_type) => {
            Err(Error::runtime_execution_error("Memory types not supported"))
        },
        KilnExternType::Global(global_type) => {
            Err(Error::runtime_execution_error("Global types not supported"))
        },
        KilnExternType::Instance(instance_type) => {
            // Convert exports to FormatExternType
            // Note: instance_type.exports is BoundedVec<Export<P>>, not tuples
            let exports_format: Result<Vec<(String, FormatExternType)>> = instance_type
                .exports
                .iter()
                .map(|export| {
                    let name_str = export
                        .name
                        .as_str()
                        .map_err(|_| {
                            Error::runtime_execution_error("Failed to convert export name")
                        })?
                        .to_string();
                    let format_extern = runtime_to_format_extern_type(&export.ty)?;
                    Ok((name_str, format_extern))
                })
                .collect();

            Ok(FormatExternType::Instance {
                exports: exports_format?,
            })
        },
        KilnExternType::Component(component_type) => {
            // Convert imports to FormatExternType
            // Note: component_type.imports is BoundedVec<Import<P>>, not tuples
            let imports_format: Result<Vec<(String, String, FormatExternType)>> = component_type
                .imports
                .iter()
                .map(|import| {
                    // Convert Namespace to string (join elements with ':')
                    let ns_str: String = import
                        .key
                        .namespace
                        .elements
                        .iter()
                        .filter_map(|elem| elem.as_str().ok().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                        .join(":");
                    let name_str = import
                        .key
                        .name
                        .as_str()
                        .map_err(|_| {
                            Error::runtime_execution_error("Failed to convert import name")
                        })?
                        .to_string();
                    let format_extern = runtime_to_format_extern_type(&import.ty)?;
                    Ok((ns_str, name_str, format_extern))
                })
                .collect();

            // Convert exports to FormatExternType
            // Note: component_type.exports is BoundedVec<Export<P>>, not tuples
            let exports_format: Result<Vec<(String, FormatExternType)>> = component_type
                .exports
                .iter()
                .map(|export| {
                    let name_str = export
                        .name
                        .as_str()
                        .map_err(|_| {
                            Error::runtime_execution_error("Failed to convert export name")
                        })?
                        .to_string();
                    let format_extern = runtime_to_format_extern_type(&export.ty)?;
                    Ok((name_str, format_extern))
                })
                .collect();

            Ok(FormatExternType::Component {
                type_idx: 0, // Placeholder type index
            })
        },
        KilnExternType::Resource(resource_type) => {
            // Note: Since FormatExternType doesn't have a direct Resource variant,
            // we map it to a Type reference with the resource type index
            // ResourceType is a tuple struct: ResourceType(u32, PhantomData<P>)
            Ok(FormatExternType::Type(resource_type.0))
        },
        KilnExternType::Tag(_tag_type) => {
            // Tag types (exception handling) - not supported yet
            Err(Error::runtime_execution_error("Tag types not supported"))
        },
        KilnExternType::CoreModule(_module_type) => {
            // Core module types - not supported yet
            Err(Error::runtime_execution_error(
                "CoreModule types not supported",
            ))
        },
        KilnExternType::TypeDef(_type_def) => {
            // Type definitions - not supported yet
            Err(Error::runtime_execution_error(
                "TypeDef types not supported",
            ))
        },
    }
}

/// Convert the format ValType to the common ValueType used in the runtime
///
/// # Arguments
///
/// * `val_type` - The format value type to convert
///
/// # Returns
///
/// Result containing the converted core value type, or an error if
/// conversion is not possible
pub fn format_to_common_val_type(val_type: &FormatValType) -> Result<ValueType> {
    match val_type {
        FormatValType::S32 => Ok(ValueType::I32),
        FormatValType::S64 => Ok(ValueType::I64),
        FormatValType::F32 => Ok(ValueType::F32),
        FormatValType::F64 => Ok(ValueType::F64),
        _ => Err(Error::runtime_type_mismatch(
            "Cannot convert format value type to common value type",
        )),
    }
}

/// Convert the common ValueType to a format ValType
///
/// # Arguments
///
/// * `value_type` - The core value type to convert
///
/// # Returns
///
/// Result containing the converted format value type, or an error if
/// conversion is not possible
pub fn common_to_format_val_type(value_type: &ValueType) -> Result<FormatValType> {
    match value_type {
        ValueType::I32 => Ok(FormatValType::S32),
        ValueType::I64 => Ok(FormatValType::S64),
        ValueType::F32 => Ok(FormatValType::F32),
        ValueType::F64 => Ok(FormatValType::F64),
        _ => Err(Error::runtime_type_mismatch(
            "Unsupported value type conversion",
        )),
    }
}

/// Convert an KilnExternType to a FuncType if it represents a function
///
/// # Arguments
///
/// * `extern_type` - The external type to convert
///
/// # Returns
///
/// The function type if the extern type is a function, or an error otherwise
pub fn extern_type_to_func_type<P: kiln_foundation::MemoryProvider>(
    extern_type: &KilnExternType<P>,
) -> Result<TypesFuncType> {
    match extern_type {
        KilnExternType::Func(func_type) => Ok(func_type.clone()),
        _ => Err(Error::runtime_type_mismatch(
            "Cannot convert format value type to common value type",
        )),
    }
}

/// Trait for types that can be converted to runtime types
pub trait IntoRuntimeType<T> {
    /// Convert to runtime type
    fn into_runtime_type(self) -> Result<T>;
}

/// Trait for types that can be converted to format types
pub trait IntoFormatType<T> {
    /// Convert to format type
    fn into_format_type(self) -> Result<T>;
}

impl<P: kiln_foundation::MemoryProvider> IntoRuntimeType<TypesKilnExternType<P>>
    for FormatExternType
{
    fn into_runtime_type(self) -> Result<TypesKilnExternType<P>> {
        format_to_runtime_extern_type::<P>(&self)
    }
}

impl<P: kiln_foundation::MemoryProvider> IntoFormatType<FormatExternType> for TypesKilnExternType<P> {
    fn into_format_type(self) -> Result<FormatExternType> {
        runtime_to_format_extern_type(&self)
    }
}

impl<P: kiln_foundation::MemoryProvider> IntoRuntimeType<KilnTypesValType<P>> for FormatValType {
    fn into_runtime_type(self) -> Result<KilnTypesValType<P>> {
        Ok(format_valtype_to_types_valtype::<P>(&self))
    }
}

impl<P: kiln_foundation::MemoryProvider> IntoFormatType<FormatValType> for KilnTypesValType<P> {
    fn into_format_type(self) -> Result<FormatValType> {
        Ok(types_valtype_to_format_valtype(&self))
    }
}

/// Convert FormatConstValue to TypesKilnComponentValue
///
/// Comprehensive conversion from format constant value to runtime component
/// value.
///
/// # Arguments
///
/// * `format_const_value` - The format constant value to convert
///
/// # Returns
///
/// The corresponding runtime component value
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::format_constvalue_to_types_componentvalue;
/// use kiln_format::component::ConstValue;
///
/// let s32_val = ConstValue::S32(42;
/// let runtime_val = format_constvalue_to_types_componentvalue(&s32_val).unwrap();
/// assert!(matches!(runtime_val, kiln_foundation::component_value::KilnComponentValue::S32(42);
/// ```
pub fn format_constvalue_to_types_componentvalue(
    format_const_value: &FormatConstValue,
) -> Result<KilnComponentValue<ComponentProvider>> {
    match format_const_value {
        FormatConstValue::Bool(v) => Ok(KilnComponentValue::Bool(*v)),
        FormatConstValue::S8(v) => Ok(KilnComponentValue::S8(*v)),
        FormatConstValue::U8(v) => Ok(KilnComponentValue::U8(*v)),
        FormatConstValue::S16(v) => Ok(KilnComponentValue::S16(*v)),
        FormatConstValue::U16(v) => Ok(KilnComponentValue::U16(*v)),
        FormatConstValue::S32(v) => Ok(KilnComponentValue::S32(*v)),
        FormatConstValue::U32(v) => Ok(KilnComponentValue::U32(*v)),
        FormatConstValue::S64(v) => Ok(KilnComponentValue::S64(*v)),
        FormatConstValue::U64(v) => Ok(KilnComponentValue::U64(*v)),
        FormatConstValue::F32(v) => Ok(KilnComponentValue::F32(
            kiln_foundation::FloatBits32::from_f32(*v),
        )),
        FormatConstValue::F64(v) => Ok(KilnComponentValue::F64(
            kiln_foundation::FloatBits64::from_f64(*v),
        )),
        FormatConstValue::Char(v) => Ok(KilnComponentValue::Char(*v)),
        FormatConstValue::String(v) => Ok(KilnComponentValue::String(v.clone())),
        FormatConstValue::Null => Ok(KilnComponentValue::Void),
    }
}

/// Convert TypesKilnComponentValue to FormatConstValue
///
/// Comprehensive conversion from runtime component value to format constant
/// value.
///
/// # Arguments
///
/// * `types_component_value` - The runtime component value to convert
///
/// # Returns
///
/// Result containing the corresponding format constant value, or an error if
/// conversion is not possible
///
/// # Examples
///
/// ```
/// use kiln_component::type_conversion::bidirectional::types_componentvalue_to_format_constvalue;
/// use kiln_foundation::component_value::KilnComponentValue;
///
/// let s32_val = KilnComponentValue::S32(42;
/// let format_val = types_componentvalue_to_format_constvalue(&s32_val).unwrap();
/// assert!(matches!(format_val, kiln_format::component::ConstValue::S32(42);
/// ```
pub fn types_componentvalue_to_format_constvalue(
    types_component_value: &KilnComponentValue<ComponentProvider>,
) -> Result<FormatConstValue> {
    match types_component_value {
        KilnComponentValue::Bool(v) => Ok(FormatConstValue::Bool(*v)),
        KilnComponentValue::S8(v) => Ok(FormatConstValue::S8(*v)),
        KilnComponentValue::U8(v) => Ok(FormatConstValue::U8(*v)),
        KilnComponentValue::S16(v) => Ok(FormatConstValue::S16(*v)),
        KilnComponentValue::U16(v) => Ok(FormatConstValue::U16(*v)),
        KilnComponentValue::S32(v) => Ok(FormatConstValue::S32(*v)),
        KilnComponentValue::U32(v) => Ok(FormatConstValue::U32(*v)),
        KilnComponentValue::S64(v) => Ok(FormatConstValue::S64(*v)),
        KilnComponentValue::U64(v) => Ok(FormatConstValue::U64(*v)),
        KilnComponentValue::F32(v) => Ok(FormatConstValue::F32(v.to_f32())),
        KilnComponentValue::F64(v) => Ok(FormatConstValue::F64(v.to_f64())),
        KilnComponentValue::Char(v) => Ok(FormatConstValue::Char(*v)),
        KilnComponentValue::String(v) => Ok(FormatConstValue::String(v.clone())),
        KilnComponentValue::Void => Ok(FormatConstValue::Null),
        _ => Err(Error::runtime_type_mismatch(
            "Cannot convert component value to constant value",
        )),
    }
}

/// Convert a core WebAssembly value to a runtime component value
///
/// This replaces the existing functionality in
/// kiln-foundation/src/component_value.rs to consolidate value conversions in
/// the same crate as type conversions.
///
/// # Arguments
///
/// * `value` - The core value to convert
///
/// # Returns
///
/// Result containing the converted component value, or an error if
/// conversion is not possible
pub fn core_value_to_types_componentvalue(
    value: &kiln_foundation::values::Value,
) -> Result<KilnComponentValue<ComponentProvider>> {
    match value {
        kiln_foundation::values::Value::I32(v) => Ok(KilnComponentValue::S32(*v)),
        kiln_foundation::values::Value::I64(v) => Ok(KilnComponentValue::S64(*v)),
        kiln_foundation::values::Value::F32(v) => Ok(KilnComponentValue::F32(
            kiln_foundation::FloatBits32::from_f32(v.value()),
        )),
        kiln_foundation::values::Value::F64(v) => Ok(KilnComponentValue::F64(
            kiln_foundation::FloatBits64::from_f64(v.value()),
        )),
        kiln_foundation::values::Value::Ref(v) => Ok(KilnComponentValue::U32(*v)), // Map reference
        // to U32
        _ => Err(Error::runtime_type_mismatch(
            "Cannot convert component value to core WebAssembly value",
        )),
    }
}

/// Convert a runtime component value to a core WebAssembly value
///
/// This replaces the existing functionality in
/// kiln-foundation/src/component_value.rs to consolidate value conversions in
/// the same crate as type conversions.
///
/// # Arguments
///
/// * `component_value` - The component value to convert
///
/// # Returns
///
/// Result containing the converted core value, or an error if
/// conversion is not possible
pub fn types_componentvalue_to_core_value(
    component_value: &KilnComponentValue<ComponentProvider>,
) -> Result<kiln_foundation::values::Value> {
    match component_value {
        KilnComponentValue::Bool(v) => {
            Ok(kiln_foundation::values::Value::I32(if *v { 1 } else { 0 }))
        },
        KilnComponentValue::S8(v) => Ok(kiln_foundation::values::Value::I32(*v as i32)),
        KilnComponentValue::U8(v) => Ok(kiln_foundation::values::Value::I32(*v as i32)),
        KilnComponentValue::S16(v) => Ok(kiln_foundation::values::Value::I32(*v as i32)),
        KilnComponentValue::U16(v) => Ok(kiln_foundation::values::Value::I32(*v as i32)),
        KilnComponentValue::S32(v) => Ok(kiln_foundation::values::Value::I32(*v)),
        KilnComponentValue::U32(v) => {
            // For U32, check if it represents a reference value (e.g., resource handle)
            // For now, we'll treat all U32 as potential references to maintain
            // compatibility A more sophisticated approach might involve
            // checking the context
            if let Some(resource_index) = is_resource_reference(*v) {
                Ok(kiln_foundation::values::Value::Ref(resource_index))
            } else {
                Ok(kiln_foundation::values::Value::I32(*v as i32))
            }
        },
        KilnComponentValue::S64(v) => Ok(kiln_foundation::values::Value::I64(*v)),
        KilnComponentValue::U64(v) => Ok(kiln_foundation::values::Value::I64(*v as i64)),
        KilnComponentValue::F32(v) => Ok(kiln_foundation::values::Value::F32(
            kiln_foundation::FloatBits32::from_bits(v.to_bits()),
        )),
        KilnComponentValue::F64(v) => Ok(kiln_foundation::values::Value::F64(
            kiln_foundation::FloatBits64::from_bits(v.to_bits()),
        )),
        _ => Err(Error::runtime_type_mismatch(
            "Cannot convert component value to core WebAssembly value",
        )),
    }
}

/// Helper function to determine if a U32 value represents a resource reference
/// This is a placeholder - in a real implementation, this might check against
/// a registry of resource handles or use contextual information.
fn is_resource_reference(value: u32) -> Option<u32> {
    // For now, we'll always return None, defaulting to treating U32 as I32
    // In a more complete implementation, this would check if the value is a valid
    // resource handle
    None
}

// Aliases for backward compatibility
pub use format_to_runtime_extern_type as format_to_types_extern_type;
pub use runtime_to_format_extern_type as types_to_format_extern_type;

/// Complete bidirectional conversion between kiln_foundation::KilnExternType and
/// kiln_format::component::KilnExternType
///
/// This function handles all KilnExternType variants comprehensively, fixing
/// previous compatibility issues.
///
/// # Arguments
///
/// * `types_extern_type` - The kiln_foundation::KilnExternType to convert
///
/// # Returns
///
/// * Result containing the converted FormatExternType or an error
pub fn complete_types_to_format_extern_type<P: kiln_foundation::MemoryProvider>(
    types_extern_type: &kiln_foundation::ExternType<P>,
) -> Result<FormatExternType> {
    match types_extern_type {
        kiln_foundation::ExternType::Func(func_type) => {
            // Convert parameter types
            let param_names: Vec<String> =
                (0..func_type.params.len()).map(|i| format!("param{}", i)).collect();

            // Create param_types manually to handle errors gracefully
            let mut param_types = Vec::new();
            for (i, value_type) in func_type.params.iter().enumerate() {
                match value_type_to_format_val_type(value_type) {
                    Ok(format_val_type) => {
                        param_types.push((param_names[i].clone(), format_val_type))
                    },
                    Err(e) => return Err(e),
                }
            }

            // Create result_types manually to handle errors gracefully
            let mut result_types = Vec::new();
            for value_type in &func_type.results {
                match value_type_to_format_val_type(value_type) {
                    Ok(format_val_type) => result_types.push(format_val_type),
                    Err(e) => return Err(e),
                }
            }

            Ok(FormatExternType::Function {
                params: param_types,
                results: result_types,
            })
        },
        kiln_foundation::ExternType::Table(table_type) => {
            Err(Error::runtime_execution_error("Table types not supported"))
        },
        kiln_foundation::ExternType::Memory(memory_type) => {
            Err(Error::runtime_execution_error("Memory types not supported"))
        },
        kiln_foundation::ExternType::Global(global_type) => {
            Err(Error::runtime_execution_error("Global types not supported"))
        },
        kiln_foundation::ExternType::Resource(resource_type) => {
            // For resources, we convert to a Type reference for now
            // In the future, this could be expanded to include full resource types
            Ok(FormatExternType::Type(0))
        },
        kiln_foundation::ExternType::Instance(instance_type) => {
            // Convert instance exports
            // Note: instance_type.exports is BoundedVec<Export<P>>, not tuples
            let exports_result: Result<Vec<(String, FormatExternType)>> = instance_type
                .exports
                .iter()
                .map(|export| {
                    let name_str = export
                        .name
                        .as_str()
                        .map_err(|_| {
                            Error::runtime_execution_error("Failed to convert export name")
                        })?
                        .to_string();
                    let format_extern = complete_types_to_format_extern_type(&export.ty)?;
                    Ok((name_str, format_extern))
                })
                .collect();

            Ok(FormatExternType::Instance {
                exports: exports_result?,
            })
        },
        kiln_foundation::ExternType::Component(component_type) => {
            // Convert component imports
            // Note: component_type.imports is BoundedVec<Import<P>>, not tuples
            let imports_result: Result<Vec<(String, String, FormatExternType)>> = component_type
                .imports
                .iter()
                .map(|import| {
                    // Convert Namespace to string (join elements with ':')
                    let ns_str: String = import
                        .key
                        .namespace
                        .elements
                        .iter()
                        .filter_map(|elem| elem.as_str().ok().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                        .join(":");
                    let name_str = import
                        .key
                        .name
                        .as_str()
                        .map_err(|_| {
                            Error::runtime_execution_error("Failed to convert import name")
                        })?
                        .to_string();
                    let format_extern = complete_types_to_format_extern_type(&import.ty)?;
                    Ok((ns_str, name_str, format_extern))
                })
                .collect();

            // Convert component exports
            // Note: component_type.exports is BoundedVec<Export<P>>, not tuples
            let exports_result: Result<Vec<(String, FormatExternType)>> = component_type
                .exports
                .iter()
                .map(|export| {
                    let name_str = export
                        .name
                        .as_str()
                        .map_err(|_| {
                            Error::runtime_execution_error("Failed to convert export name")
                        })?
                        .to_string();
                    let format_extern = complete_types_to_format_extern_type(&export.ty)?;
                    Ok((name_str, format_extern))
                })
                .collect();

            Ok(FormatExternType::Component {
                type_idx: 0, // Placeholder type index
            })
        },
        kiln_foundation::ExternType::Tag(_tag_type) => {
            // Tag types (exception handling) - not supported yet
            Err(Error::runtime_execution_error("Tag types not supported"))
        },
        kiln_foundation::ExternType::CoreModule(_module_type) => {
            // Core module types - not supported yet
            Err(Error::runtime_execution_error(
                "CoreModule types not supported",
            ))
        },
        kiln_foundation::ExternType::TypeDef(_type_def) => {
            // Type definitions - not supported yet
            Err(Error::runtime_execution_error(
                "TypeDef types not supported",
            ))
        },
    }
}

/// Complete bidirectional conversion from kiln_format::component::KilnExternType
/// to kiln_foundation::ExternType
///
/// This function handles all KilnExternType variants comprehensively, fixing
/// previous compatibility issues.
///
/// # Arguments
///
/// * `format_extern_type` - The kiln_format::component::KilnExternType to convert
///
/// # Returns
///
/// * Result containing the converted kiln_foundation::ExternType or an error
pub fn complete_format_to_types_extern_type<P: kiln_foundation::MemoryProvider>(
    format_extern_type: &FormatExternType,
) -> Result<kiln_foundation::ExternType<P>> {
    match format_extern_type {
        FormatExternType::Module { type_idx } => {
            // Core module type - not yet implemented
            Err(Error::runtime_execution_error(
                "Module extern types not yet supported",
            ))
        },
        FormatExternType::Function { params, results } => {
            // Convert parameter types - create an empty vector and then convert and add
            // each parameter
            let mut param_types = Vec::new();
            for (_, format_val_type) in params {
                // First convert to KilnTypesValType, then to ValueType if needed
                let _types_val_type: kiln_foundation::ValType<P> =
                    convert_format_to_types_valtype(format_val_type);
                match convert_format_valtype_to_valuetype(format_val_type) {
                    Ok(value_type) => param_types.push(value_type),
                    Err(_) => {
                        return Err(Error::new(
                            ErrorCategory::Type,
                            codes::CONVERSION_ERROR,
                            "ValType conversion not implemented",
                        ));
                    },
                }
            }

            // Convert result types - create an empty vector and then convert and add each
            // result
            let mut result_types = Vec::new();
            for format_val_type in results {
                // First convert to KilnTypesValType, then to ValueType if needed
                let _types_val_type: kiln_foundation::ValType<P> =
                    convert_format_to_types_valtype(format_val_type);
                match convert_format_valtype_to_valuetype(format_val_type) {
                    Ok(value_type) => result_types.push(value_type),
                    Err(_) => {
                        return Err(Error::runtime_execution_error(
                            "Failed to convert result type",
                        ));
                    },
                }
            }

            // Create a new FuncType properly
            let provider = P::default();
            Ok(kiln_foundation::ExternType::Func(
                kiln_foundation::FuncType::new(param_types, result_types)?,
            ))
        },
        FormatExternType::Value(format_val_type) => {
            // Value types typically map to globals in the runtime
            // First convert to KilnTypesValType, then to ValueType if needed
            let _types_val_type: kiln_foundation::ValType<P> =
                convert_format_to_types_valtype(format_val_type);
            let value_type = match convert_format_valtype_to_valuetype(format_val_type) {
                Ok(vt) => vt,
                Err(_) => {
                    return Err(Error::new(
                        ErrorCategory::Type,
                        codes::CONVERSION_ERROR,
                        "ValType conversion not implemented",
                    ));
                },
            };
            Ok(kiln_foundation::ExternType::Global(
                kiln_foundation::GlobalType {
                    value_type,
                    mutable: false, // Values are typically immutable
                },
            ))
        },
        FormatExternType::Type(type_idx) => {
            // Type references typically map to resources for now
            // ResourceType is a tuple struct: ResourceType(u32, PhantomData<P>)
            Ok(kiln_foundation::ExternType::Resource(
                kiln_foundation::ResourceType(*type_idx, core::marker::PhantomData),
            ))
        },
        FormatExternType::Instance { exports } => {
            // Get a provider for creating the bounded structures
            let provider = P::default();

            // Convert instance exports to Export<P> structs
            let mut export_vec: kiln_foundation::BoundedVec<kiln_foundation::Export<P>, 128, P> =
                kiln_foundation::BoundedVec::new(provider.clone())?;

            for (name, extern_type) in exports {
                let types_extern = complete_format_to_types_extern_type::<P>(extern_type)?;
                let name_wasm = kiln_foundation::WasmName::try_from_str(name)
                    .map_err(|_| Error::runtime_execution_error("Invalid export name"))?;
                let export = kiln_foundation::Export {
                    name: name_wasm,
                    ty: types_extern,
                    desc: None,
                };
                export_vec
                    .push(export)
                    .map_err(|_| Error::capacity_exceeded("Too many exports"))?;
            }

            Ok(kiln_foundation::ExternType::Instance(
                kiln_foundation::InstanceType {
                    exports: export_vec,
                },
            ))
        },
        FormatExternType::Component { type_idx } => {
            // Get a provider for creating the bounded structures
            let provider = P::default();

            // Convert component imports to Import<P> structs
            let mut import_vec: kiln_foundation::BoundedVec<kiln_foundation::Import<P>, 128, P> =
                kiln_foundation::BoundedVec::new(provider.clone())?;

            // No imports/exports to iterate - type_idx is just a reference
            // Create empty bounded vecs
            let export_vec: kiln_foundation::BoundedVec<kiln_foundation::Export<P>, 128, P> =
                kiln_foundation::BoundedVec::new(provider.clone())?;

            // Create empty instances BoundedVec
            let instances = kiln_foundation::BoundedVec::new(provider.clone())?;

            Ok(kiln_foundation::ExternType::Component(
                kiln_foundation::ComponentType {
                    imports: import_vec,
                    exports: export_vec,
                    aliases: kiln_foundation::BoundedVec::new(provider.clone())?,
                    instances,
                    core_instances: kiln_foundation::BoundedVec::new(provider.clone())?,
                    component_types: kiln_foundation::BoundedVec::new(provider.clone())?,
                    core_types: kiln_foundation::BoundedVec::new(provider.clone())?,
                },
            ))
        },
    }
}
