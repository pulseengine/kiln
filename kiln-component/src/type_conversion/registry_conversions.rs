/// Registry-based type conversions
///
/// This module implements conversions between format and runtime types using
/// the TypeConversionRegistry, providing a consistent and extensible approach
/// to type conversion.
use wrt_format::component::{ComponentTypeDefinition, ExternType as FormatExternType};
// Note: wrt_format::component::ValType is actually wrt_foundation::component_value::ValType in std mode
// So we can't use it as an alias - we need to use the foundation type directly
type FormatValType<P> = wrt_foundation::component_value::ValType<P>;
use wrt_foundation::{
    ExternType as TypesExternType,
    component::{ComponentType, InstanceType},
    component_value::ValType as TypesValType,
    types::{FuncType, ValueType},
};

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use std::string::String;

// For no_std, override prelude's bounded::BoundedVec with StaticVec
#[cfg(not(feature = "std"))]
use wrt_foundation::collections::StaticVec as BoundedVec;

use super::{
    registry::{ConversionError, ConversionErrorKind, TypeConversionRegistry},
    wrappers::{
        FormatComponentType, FormatInstanceType, RuntimeComponentType, RuntimeInstanceType,
    },
};
// Import only what we need from prelude to avoid ValType name collision
use crate::prelude::{ComponentProvider, CrateId};

/// Register ValType conversions in the TypeConversionRegistry
pub fn register_valtype_conversions(registry: &mut TypeConversionRegistry) {
    // Format ValType to Types ValType - primitive types (they're the same type in std mode)
    registry.register(
        |format_val_type: &FormatValType<ComponentProvider>| -> core::result::Result<TypesValType<ComponentProvider>, ConversionError> {
            // Since FormatValType and TypesValType are the same type in std mode, just clone
            Ok(format_val_type.clone())
        },
    );

    // Types ValType to Format ValType - primitive types (same type in std mode)
    registry.register(
        |types_val_type: &TypesValType<ComponentProvider>| -> core::result::Result<FormatValType<ComponentProvider>, ConversionError> {
            // Since FormatValType and TypesValType are the same type in std mode, just clone
            Ok(types_val_type.clone())
        },
    );

    // ValueType to FormatValType conversion
    registry.register(
        |value_type: &ValueType| -> core::result::Result<FormatValType<ComponentProvider>, ConversionError> {
            match value_type {
                ValueType::I32 => Ok(FormatValType::<ComponentProvider>::S32),
                ValueType::I64 => Ok(FormatValType::<ComponentProvider>::S64),
                ValueType::F32 => Ok(FormatValType::<ComponentProvider>::F32),
                ValueType::F64 => Ok(FormatValType::<ComponentProvider>::F64),
                ValueType::V128 => Err(ConversionError {
                    kind:        ConversionErrorKind::InvalidVariant,
                    source_type: "ValueType::V128",
                    target_type: "FormatValType",
                    context:     Some(String::from(
                        "V128 SIMD type not supported in component model"
                    )),
                    source:      None,
                }),
                ValueType::I16x8 => Err(ConversionError {
                    kind:        ConversionErrorKind::InvalidVariant,
                    source_type: "ValueType::I16x8",
                    target_type: "FormatValType",
                    context:     Some(String::from(
                        "I16x8 SIMD type not supported in component model"
                    )),
                    source:      None,
                }),
                ValueType::FuncRef | ValueType::ExternRef | ValueType::NullFuncRef => Err(ConversionError {
                    kind:        ConversionErrorKind::InvalidVariant,
                    source_type: "ValueType::FuncRef/ExternRef/NullFuncRef",
                    target_type: "FormatValType",
                    context:     Some(String::from(
                        "Reference types cannot be directly converted to component format types"
                    )),
                    source:      None,
                }),
                ValueType::StructRef(_) | ValueType::ArrayRef(_) | ValueType::ExnRef
                | ValueType::I31Ref | ValueType::AnyRef | ValueType::EqRef
                | ValueType::TypedFuncRef(_, _) => Err(ConversionError {
                    kind:        ConversionErrorKind::InvalidVariant,
                    source_type: "ValueType::StructRef/ArrayRef/ExnRef/GC",
                    target_type: "FormatValType",
                    context:     Some(String::from(
                        "GC/EH types not supported in component model"
                    )),
                    source:      None,
                }),
            }
        },
    );

    // FormatValType to ValueType conversion
    registry.register(
        |format_val_type: &FormatValType<ComponentProvider>| -> core::result::Result<ValueType, ConversionError> {
            match format_val_type {
                FormatValType::S32 => Ok(ValueType::I32),
                FormatValType::S64 => Ok(ValueType::I64),
                FormatValType::F32 => Ok(ValueType::F32),
                FormatValType::F64 => Ok(ValueType::F64),
                _ => Err(ConversionError {
                    kind:        ConversionErrorKind::InvalidVariant,
                    source_type: "FormatValType",
                    target_type: "ValueType",
                    context:     Some("Component not found".to_string()),
                    source:      None,
                }),
            }
        },
    );

    // ValueType to ValType conversion
    registry.register(
        |value_type: &ValueType| -> core::result::Result<TypesValType<ComponentProvider>, ConversionError> {
            match value_type {
                ValueType::I32 => Ok(TypesValType::<ComponentProvider>::S32),
                ValueType::I64 => Ok(TypesValType::<ComponentProvider>::S64),
                ValueType::F32 => Ok(TypesValType::<ComponentProvider>::F32),
                ValueType::F64 => Ok(TypesValType::<ComponentProvider>::F64),
                ValueType::V128 => Err(ConversionError {
                    kind:        ConversionErrorKind::InvalidVariant,
                    source_type: "ValueType::V128",
                    target_type: "TypesValType",
                    context:     Some(String::from(
                        "V128 SIMD type not supported in component model"
                    )),
                    source:      None,
                }),
                ValueType::I16x8 => Err(ConversionError {
                    kind:        ConversionErrorKind::InvalidVariant,
                    source_type: "ValueType::I16x8",
                    target_type: "TypesValType",
                    context:     Some(String::from(
                        "I16x8 SIMD type not supported in component model"
                    )),
                    source:      None,
                }),
                ValueType::FuncRef => Ok(TypesValType::<ComponentProvider>::Own(0)), // Default to resource type 0
                ValueType::NullFuncRef => Ok(TypesValType::<ComponentProvider>::Own(0)), // Bottom funcref type
                ValueType::ExternRef => Ok(TypesValType::<ComponentProvider>::Ref(0)), // Default to type index 0
                ValueType::StructRef(_) | ValueType::ArrayRef(_) | ValueType::ExnRef
                | ValueType::I31Ref | ValueType::AnyRef | ValueType::EqRef
                | ValueType::TypedFuncRef(_, _) => Err(ConversionError {
                    kind:        ConversionErrorKind::InvalidVariant,
                    source_type: "ValueType::StructRef/ArrayRef/ExnRef/GC",
                    target_type: "TypesValType",
                    context:     Some(String::from(
                        "GC/EH types not supported in component model"
                    )),
                    source:      None,
                }),
            }
        },
    );
}

/// Register ExternType conversions in the TypeConversionRegistry
pub fn register_externtype_conversions(registry: &mut TypeConversionRegistry) {
    // Minimal implementation to support our simple tests
}

/// Register ComponentType and InstanceType conversions in the
/// TypeConversionRegistry
pub fn register_component_instancetype_conversions(registry: &mut TypeConversionRegistry) {}
