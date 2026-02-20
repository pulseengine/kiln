// Component format conversion module
//
// This module provides direct conversion between binary format types and
// runtime types without requiring intermediate conversions.

use wrt_error::Result;
use wrt_foundation::ValueType;

// Import the properly re-exported ValType
use crate::component::FormatValType;

// Create a wrapper type to avoid orphan rule violations - using clean types
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct ValTypeWrapper(pub FormatValType);

// For no_std without alloc, use simplified conversion without wrappers
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub struct ValTypeWrapper(pub ValueType);

// Implement a conversion function from FormatValType to ValueType
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn format_val_type_to_value_type(format_type: &FormatValType) -> Result<ValueType> {
    match format_type {
        FormatValType::S8
        | FormatValType::U8
        | FormatValType::S16
        | FormatValType::U16
        | FormatValType::S32
        | FormatValType::U32
        | FormatValType::Bool
        | FormatValType::Char
        | FormatValType::Flags(_)
        | FormatValType::Enum(_)
        | FormatValType::ErrorContext => Ok(ValueType::I32),

        FormatValType::S64 | FormatValType::U64 => Ok(ValueType::I64),

        FormatValType::F32 => Ok(ValueType::F32),
        FormatValType::F64 => Ok(ValueType::F64),

        // References and handles
        FormatValType::String
        | FormatValType::Record(_)
        | FormatValType::Variant(_)
        | FormatValType::List(_)
        | FormatValType::FixedList(_, _)
        | FormatValType::Tuple(_)
        | FormatValType::Option(_)
        | FormatValType::Result(_)
        | FormatValType::Own(_)
        | FormatValType::Borrow(_)
        | FormatValType::Ref(_) => Ok(ValueType::ExternRef),

        FormatValType::Void => Ok(ValueType::I32), // Map Void to I32 as a fallback
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
pub fn format_val_type_to_value_type(format_type: &ValueType) -> Result<ValueType> {
    // In no_std without alloc, we use simplified conversion
    Ok(*format_type)
}

// Implement a conversion function from ValueType to FormatValType
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn value_type_to_format_val_type(value_type: &ValueType) -> Result<FormatValType> {
    match value_type {
        ValueType::I32 => Ok(FormatValType::S32),
        ValueType::I64 => Ok(FormatValType::S64),
        ValueType::F32 => Ok(FormatValType::F32),
        ValueType::F64 => Ok(FormatValType::F64),
        ValueType::V128 => Err(wrt_error::Error::runtime_execution_error(
            "V128 type not supported in component model",
        )),
        ValueType::I16x8 => Err(wrt_error::Error::new(
            wrt_error::ErrorCategory::Parse,
            wrt_error::codes::UNIMPLEMENTED,
            "I16x8 type not supported in component model",
        )),
        ValueType::FuncRef => Ok(FormatValType::Own(0)), // Map to handle
        ValueType::NullFuncRef => Ok(FormatValType::Own(0)), // Map bottom funcref to handle
        ValueType::ExternRef => Ok(FormatValType::Own(0)), // Map to handle
        ValueType::StructRef(_) => Ok(FormatValType::Own(0)), // Map struct reference to handle
        ValueType::ArrayRef(_) => Ok(FormatValType::Own(0)), // Map array reference to handle
        ValueType::ExnRef => Ok(FormatValType::Own(0)),  // Map exception reference to handle
        ValueType::I31Ref => Ok(FormatValType::S32),     // i31 fits in s32
        ValueType::AnyRef => Ok(FormatValType::Own(0)),  // Map any reference to handle
        ValueType::EqRef => Ok(FormatValType::Own(0)),   // Map eq reference to handle
        ValueType::TypedFuncRef(_, _) => Ok(FormatValType::Own(0)), // Map typed funcref to handle
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
pub fn value_type_to_format_val_type(value_type: &ValueType) -> Result<ValueType> {
    // In no_std without alloc, we use simplified conversion
    Ok(*value_type)
}

// Map a core WebAssembly ValueType to a Component Model ValType
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn map_wasm_type_to_component(ty: ValueType) -> wrt_error::Result<FormatValType> {
    match ty {
        ValueType::I32 => Ok(FormatValType::S32),
        ValueType::I64 => Ok(FormatValType::S64),
        ValueType::F32 => Ok(FormatValType::F32),
        ValueType::F64 => Ok(FormatValType::F64),
        ValueType::V128 => Err(wrt_error::Error::runtime_execution_error(
            "V128 type not supported in component model",
        )),
        ValueType::I16x8 => Err(wrt_error::Error::new(
            wrt_error::ErrorCategory::Parse,
            wrt_error::codes::UNIMPLEMENTED,
            "I16x8 type not supported in component model",
        )),
        ValueType::FuncRef => Ok(FormatValType::Own(0)), // Map to handle
        ValueType::NullFuncRef => Ok(FormatValType::Own(0)), // Map bottom funcref to handle
        ValueType::ExternRef => Ok(FormatValType::Own(0)), // Map to handle
        ValueType::StructRef(_) => Ok(FormatValType::Own(0)), // Map struct reference to handle
        ValueType::ArrayRef(_) => Ok(FormatValType::Own(0)), // Map array reference to handle
        ValueType::ExnRef => Ok(FormatValType::Own(0)),  // Map exception reference to handle
        ValueType::I31Ref => Ok(FormatValType::S32),     // i31 fits in s32
        ValueType::AnyRef => Ok(FormatValType::Own(0)),  // Map any reference to handle
        ValueType::EqRef => Ok(FormatValType::Own(0)),   // Map eq reference to handle
        ValueType::TypedFuncRef(_, _) => Ok(FormatValType::Own(0)), // Map typed funcref to handle
    }
}

#[cfg(not(any(feature = "std")))]
pub fn map_wasm_type_to_component<
    P: wrt_foundation::MemoryProvider + Default + Clone + PartialEq + Eq,
>(
    ty: ValueType,
) -> Result<FormatValType<P>, wrt_error::Error> {
    match ty {
        ValueType::I32 => Ok(FormatValType::S32),
        ValueType::I64 => Ok(FormatValType::S64),
        ValueType::F32 => Ok(FormatValType::F32),
        ValueType::F64 => Ok(FormatValType::F64),
        ValueType::V128 => Err(wrt_error::Error::runtime_execution_error(
            "V128 type not supported in component model",
        )),
        ValueType::I16x8 => Err(wrt_error::Error::new(
            wrt_error::ErrorCategory::Parse,
            wrt_error::codes::UNIMPLEMENTED,
            "I16x8 type not supported in component model",
        )),
        ValueType::FuncRef => Ok(FormatValType::Own(0)), // Map to handle
        ValueType::NullFuncRef => Ok(FormatValType::Own(0)), // Map bottom funcref to handle
        ValueType::ExternRef => Ok(FormatValType::Own(0)), // Map to handle
        ValueType::StructRef(_) => Ok(FormatValType::Own(0)), // Map struct reference to handle
        ValueType::ArrayRef(_) => Ok(FormatValType::Own(0)), // Map array reference to handle
        ValueType::ExnRef => Ok(FormatValType::Own(0)),  // Map exception reference to handle
        ValueType::I31Ref => Ok(FormatValType::S32),     // i31 fits in s32
        ValueType::AnyRef => Ok(FormatValType::Own(0)),  // Map any reference to handle
        ValueType::EqRef => Ok(FormatValType::Own(0)),   // Map eq reference to handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_value_type_conversion() {
        // Test basic primitive types
        let s32_val = FormatValType::S32;
        let i32_val = format_val_type_to_value_type(&s32_val).unwrap();
        assert_eq!(i32_val, ValueType::I32);

        let f64_val = FormatValType::F64;
        let f64_runtime = format_val_type_to_value_type(&f64_val).unwrap();
        assert_eq!(f64_runtime, ValueType::F64);

        // Test complex types (all map to ExternRef)
        let string_val = FormatValType::String;
        let string_runtime = format_val_type_to_value_type(&string_val).unwrap();
        assert_eq!(string_runtime, ValueType::ExternRef);

        // Test roundtrip conversion for basic types
        let i32_val = ValueType::I32;
        let format_val = value_type_to_format_val_type(&i32_val).unwrap();
        let roundtrip = format_val_type_to_value_type(&format_val).unwrap();
        assert_eq!(i32_val, roundtrip);
    }
}
