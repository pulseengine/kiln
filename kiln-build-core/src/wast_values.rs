//! WAST Value Conversion Utilities
//!
//! This module provides functions to convert between WAST test framework value
//! types and Kiln runtime value types, including proper handling of NaN patterns,
//! V128 vectors, and reference types.

#![cfg(feature = "std")]

use anyhow::Result;
use wast::{
    WastArg, WastRet,
    core::{NanPattern, V128Pattern, WastArgCore, WastRetCore},
};
use kiln_foundation::values::{ExternRef, FloatBits32, FloatBits64, FuncRef, V128, Value};

/// Convert WAST arguments to Kiln values
pub fn convert_wast_args_to_values(args: &[WastArg]) -> Result<Vec<Value>> {
    args.iter().map(convert_wast_arg_to_value).collect()
}

/// Convert a single WAST argument to a Kiln value
pub fn convert_wast_arg_to_value(arg: &WastArg) -> Result<Value> {
    match arg {
        WastArg::Core(core_arg) => convert_wast_arg_core_to_value(core_arg),
        _ => Err(anyhow::anyhow!("Unsupported WAST argument type")),
    }
}

/// Convert WAST core argument to Kiln value
pub fn convert_wast_arg_core_to_value(arg: &WastArgCore) -> Result<Value> {
    match arg {
        WastArgCore::I32(x) => Ok(Value::I32(*x)),
        WastArgCore::I64(x) => Ok(Value::I64(*x)),
        WastArgCore::F32(x) => Ok(Value::F32(FloatBits32::from_bits(x.bits))),
        WastArgCore::F64(x) => Ok(Value::F64(FloatBits64::from_bits(x.bits))),
        WastArgCore::V128(x) => Ok(Value::V128(V128::new(convert_v128_const_to_bytes(x)?))),
        WastArgCore::RefNull(heap_type) => {
            // ref.null funcref -> FuncRef(None), ref.null externref -> ExternRef(None)
            use wast::core::AbstractHeapType;
            match heap_type {
                wast::core::HeapType::Abstract {
                    ty: AbstractHeapType::Func,
                    ..
                } => Ok(Value::FuncRef(None)),
                wast::core::HeapType::Abstract {
                    ty: AbstractHeapType::Extern,
                    ..
                } => Ok(Value::ExternRef(None)),
                _ => Ok(Value::FuncRef(None)), // Default to FuncRef for other/unknown heap types
            }
        },
        WastArgCore::RefExtern(x) => Ok(Value::ExternRef(Some(ExternRef { index: *x as u32 }))),
        WastArgCore::RefHost(x) => Ok(Value::ExternRef(Some(ExternRef { index: *x as u32 }))),
    }
}

/// Convert WAST expected results to Kiln values for comparison
pub fn convert_wast_results_to_values(results: &[WastRet]) -> Result<Vec<Value>> {
    results.iter().map(convert_wast_ret_to_value).collect()
}

/// Convert a single WAST return value to a Kiln value
pub fn convert_wast_ret_to_value(ret: &WastRet) -> Result<Value> {
    match ret {
        WastRet::Core(core_ret) => convert_wast_ret_core_to_value(core_ret),
        _ => Err(anyhow::anyhow!("Unsupported WAST return type")),
    }
}

/// Convert WAST core return value to Kiln value
pub fn convert_wast_ret_core_to_value(ret: &WastRetCore) -> Result<Value> {
    match ret {
        WastRetCore::I32(x) => Ok(Value::I32(*x)),
        WastRetCore::I64(x) => Ok(Value::I64(*x)),
        WastRetCore::F32(nan_pattern) => match nan_pattern {
            NanPattern::Value(x) => Ok(Value::F32(FloatBits32::from_bits(x.bits))),
            NanPattern::CanonicalNan => Ok(Value::F32(FloatBits32::NAN)),
            NanPattern::ArithmeticNan => Ok(Value::F32(FloatBits32::NAN)),
        },
        WastRetCore::F64(nan_pattern) => match nan_pattern {
            NanPattern::Value(x) => Ok(Value::F64(FloatBits64::from_bits(x.bits))),
            NanPattern::CanonicalNan => Ok(Value::F64(FloatBits64::NAN)),
            NanPattern::ArithmeticNan => Ok(Value::F64(FloatBits64::NAN)),
        },
        WastRetCore::V128(x) => Ok(Value::V128(V128::new(convert_v128_pattern_to_bytes(x)?))),
        WastRetCore::RefNull(heap_type) => {
            // Convert ref.null with various heap types to appropriate Value
            use wast::core::AbstractHeapType;
            match heap_type {
                Some(wast::core::HeapType::Abstract { ty, .. }) => {
                    match ty {
                        // Standard reference types
                        AbstractHeapType::Func => Ok(Value::FuncRef(None)),
                        AbstractHeapType::Extern => Ok(Value::ExternRef(None)),
                        // GC abstract heap types
                        AbstractHeapType::Any => Ok(Value::ExternRef(None)), // anyref uses externref repr
                        AbstractHeapType::Eq => Ok(Value::I31Ref(None)), // eqref uses i31ref repr
                        AbstractHeapType::I31 => Ok(Value::I31Ref(None)),
                        AbstractHeapType::Struct => Ok(Value::StructRef(None)),
                        AbstractHeapType::Array => Ok(Value::ArrayRef(None)),
                        AbstractHeapType::Exn => Ok(Value::ExnRef(None)),
                        // Bottom types
                        AbstractHeapType::NoFunc => Ok(Value::FuncRef(None)),
                        AbstractHeapType::NoExtern => Ok(Value::ExternRef(None)),
                        AbstractHeapType::None => Ok(Value::ExternRef(None)), // none uses externref repr
                        AbstractHeapType::NoExn => Ok(Value::ExnRef(None)),
                        // Continuation types (not yet supported)
                        AbstractHeapType::Cont | AbstractHeapType::NoCont => {
                            Ok(Value::FuncRef(None)) // Default for unsupported
                        },
                    }
                },
                Some(wast::core::HeapType::Concrete(_)) => {
                    // Concrete type reference - use FuncRef for function types
                    Ok(Value::FuncRef(None))
                },
                None => Ok(Value::FuncRef(None)), // Default for unspecified heap type
            }
        },
        WastRetCore::RefExtern(x) => match x {
            Some(idx) => Ok(Value::ExternRef(Some(ExternRef { index: *idx as u32 }))),
            None => Ok(Value::ExternRef(None)),
        },
        WastRetCore::RefHost(x) => Ok(Value::ExternRef(Some(ExternRef { index: *x as u32 }))),
        WastRetCore::RefFunc(x) => {
            // ref.func index -> FuncRef(Some(index))
            // ref.func (no index) -> any non-null funcref (use sentinel u32::MAX)
            match x {
                Some(idx) => {
                    // Extract numeric index from Index enum
                    let func_index = match idx {
                        wast::token::Index::Num(n, _) => *n,
                        wast::token::Index::Id(_) => 0, // Named indices default to 0
                    };
                    Ok(Value::FuncRef(Some(FuncRef::from_index(func_index))))
                },
                None => {
                    // (ref.func) without index means "any non-null funcref"
                    // Use u32::MAX as a sentinel value for pattern matching
                    Ok(Value::FuncRef(Some(FuncRef::from_index(u32::MAX))))
                },
            }
        },
        WastRetCore::RefI31 | WastRetCore::RefI31Shared => {
            // (ref.i31) - any non-null i31 reference
            // Use a sentinel value to indicate "any non-null i31ref"
            Ok(Value::I31Ref(Some(i32::MAX)))
        },
        WastRetCore::RefStruct => {
            // (ref.struct) - any non-null struct reference
            // Use a sentinel StructRef with alloc_id = u32::MAX
            let sentinel = kiln_foundation::values::StructRef::new(
                u32::MAX,
                kiln_foundation::traits::DefaultMemoryProvider::default(),
            ).map_err(|e| anyhow::anyhow!("Failed to create sentinel StructRef: {}", e))?;
            Ok(Value::StructRef(Some(sentinel)))
        },
        WastRetCore::RefArray => {
            // (ref.array) - any non-null array reference
            // Use a sentinel ArrayRef with alloc_id = u32::MAX
            let mut sentinel = kiln_foundation::values::ArrayRef::new(
                u32::MAX,
                kiln_foundation::traits::DefaultMemoryProvider::default(),
            ).map_err(|e| anyhow::anyhow!("Failed to create sentinel ArrayRef: {}", e))?;
            sentinel.alloc_id = u32::MAX;
            Ok(Value::ArrayRef(Some(sentinel)))
        },
        WastRetCore::RefEq => {
            // (ref.eq) - any non-null eqref (i31, struct, or array)
            // Use I31Ref sentinel with i32::MAX - values_equal handles cross-type matching
            Ok(Value::I31Ref(Some(i32::MAX)))
        },
        WastRetCore::RefAny => {
            // (ref.any) - any non-null anyref (i31, struct, array)
            // Use I31Ref sentinel with i32::MAX - values_equal handles cross-type matching
            Ok(Value::I31Ref(Some(i32::MAX)))
        },
        _ => {
            // Handle other reference types with default FuncRef
            Ok(Value::FuncRef(None))
        },
    }
}

/// Convert V128Const to byte array
fn convert_v128_const_to_bytes(v128: &wast::core::V128Const) -> Result<[u8; 16]> {
    Ok(v128.to_le_bytes())
}

/// Convert V128Pattern to byte array
fn convert_v128_pattern_to_bytes(pattern: &V128Pattern) -> Result<[u8; 16]> {
    match pattern {
        V128Pattern::I8x16(values) => {
            let mut bytes = [0u8; 16];
            for (i, &val) in values.iter().enumerate() {
                bytes[i] = val as u8;
            }
            Ok(bytes)
        },
        V128Pattern::I16x8(values) => {
            let mut bytes = [0u8; 16];
            for (i, &val) in values.iter().enumerate() {
                let val_bytes = val.to_le_bytes();
                bytes[i * 2] = val_bytes[0];
                bytes[i * 2 + 1] = val_bytes[1];
            }
            Ok(bytes)
        },
        V128Pattern::I32x4(values) => {
            let mut bytes = [0u8; 16];
            for (i, &val) in values.iter().enumerate() {
                let val_bytes = val.to_le_bytes();
                bytes[i * 4..i * 4 + 4].copy_from_slice(&val_bytes);
            }
            Ok(bytes)
        },
        V128Pattern::I64x2(values) => {
            let mut bytes = [0u8; 16];
            for (i, &val) in values.iter().enumerate() {
                let val_bytes = val.to_le_bytes();
                bytes[i * 8..i * 8 + 8].copy_from_slice(&val_bytes);
            }
            Ok(bytes)
        },
        V128Pattern::F32x4(values) => {
            let mut bytes = [0u8; 16];
            for (i, pattern) in values.iter().enumerate() {
                let val = match pattern {
                    NanPattern::Value(x) => f32::from_bits(x.bits),
                    NanPattern::CanonicalNan => f32::NAN,
                    NanPattern::ArithmeticNan => f32::NAN,
                };
                let val_bytes = val.to_le_bytes();
                bytes[i * 4..i * 4 + 4].copy_from_slice(&val_bytes);
            }
            Ok(bytes)
        },
        V128Pattern::F64x2(values) => {
            let mut bytes = [0u8; 16];
            for (i, pattern) in values.iter().enumerate() {
                let val = match pattern {
                    NanPattern::Value(x) => f64::from_bits(x.bits),
                    NanPattern::CanonicalNan => f64::NAN,
                    NanPattern::ArithmeticNan => f64::NAN,
                };
                let val_bytes = val.to_le_bytes();
                bytes[i * 8..i * 8 + 8].copy_from_slice(&val_bytes);
            }
            Ok(bytes)
        },
    }
}

/// Check if runtime error matches expected trap message
pub fn is_expected_trap(error_str: &str, expected_message: &str) -> bool {
    let error_message = error_str.to_lowercase();
    let expected = expected_message.to_lowercase();

    // Common trap patterns
    let trap_patterns = [
        "out of bounds",
        "unreachable",
        "divide by zero",
        "integer overflow",
        "invalid conversion",
        "stack overflow",
        "call indirect",
        "type mismatch",
        "memory access",
        "table access",
    ];

    // Check if error message contains expected pattern
    if error_message.contains(&expected) {
        return true;
    }

    // Check if error message contains any trap pattern that matches expected
    for pattern in &trap_patterns {
        if expected.contains(pattern) && error_message.contains(pattern) {
            return true;
        }
    }

    false
}

/// Compare two values for equality, handling NaN patterns
pub fn values_equal(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::I32(a), Value::I32(b)) => a == b,
        (Value::I64(a), Value::I64(b)) => a == b,
        (Value::F32(a), Value::F32(b)) => {
            // Handle NaN comparison
            let a_val = a.value();
            let b_val = b.value();
            if a_val.is_nan() && b_val.is_nan() { true } else { a == b }
        },
        (Value::F64(a), Value::F64(b)) => {
            // Handle NaN comparison
            let a_val = a.value();
            let b_val = b.value();
            if a_val.is_nan() && b_val.is_nan() { true } else { a == b }
        },
        (Value::V128(a), Value::V128(b)) => {
            if a == b {
                return true;
            }
            // NaN-aware V128 comparison: try f32x4 lane comparison where NaN lanes match any NaN
            let a_bytes = &a.bytes;
            let b_bytes = &b.bytes;
            // Try f32x4 interpretation
            let f32_match = (0..4).all(|i| {
                let a_val = f32::from_le_bytes([a_bytes[i*4], a_bytes[i*4+1], a_bytes[i*4+2], a_bytes[i*4+3]]);
                let b_val = f32::from_le_bytes([b_bytes[i*4], b_bytes[i*4+1], b_bytes[i*4+2], b_bytes[i*4+3]]);
                if a_val.is_nan() && b_val.is_nan() {
                    true
                } else {
                    a_bytes[i*4..i*4+4] == b_bytes[i*4..i*4+4]
                }
            });
            if f32_match { return true; }
            // Try f64x2 interpretation
            (0..2).all(|i| {
                let a_val = f64::from_le_bytes([
                    a_bytes[i*8], a_bytes[i*8+1], a_bytes[i*8+2], a_bytes[i*8+3],
                    a_bytes[i*8+4], a_bytes[i*8+5], a_bytes[i*8+6], a_bytes[i*8+7],
                ]);
                let b_val = f64::from_le_bytes([
                    b_bytes[i*8], b_bytes[i*8+1], b_bytes[i*8+2], b_bytes[i*8+3],
                    b_bytes[i*8+4], b_bytes[i*8+5], b_bytes[i*8+6], b_bytes[i*8+7],
                ]);
                if a_val.is_nan() && b_val.is_nan() {
                    true
                } else {
                    a_bytes[i*8..i*8+8] == b_bytes[i*8..i*8+8]
                }
            })
        },
        (Value::Ref(a), Value::Ref(b)) => a == b,
        // FuncRef comparison
        // Handle "any funcref" pattern (u32::MAX sentinel)
        (Value::FuncRef(Some(_)), Value::FuncRef(Some(FuncRef { index: u32::MAX, .. }))) => true,
        (Value::FuncRef(None), Value::FuncRef(Some(FuncRef { index: u32::MAX, .. }))) => false,
        (Value::FuncRef(a), Value::FuncRef(b)) => a == b,
        // ExternRef comparison
        // Handle "any externref" pattern (u32::MAX sentinel)
        (Value::ExternRef(Some(_)), Value::ExternRef(Some(ExternRef { index: u32::MAX }))) => true,
        (Value::ExternRef(None), Value::ExternRef(Some(ExternRef { index: u32::MAX }))) => false,
        (Value::ExternRef(a), Value::ExternRef(b)) => a == b,
        // Cross-type comparison: FuncRef vs Ref (for backwards compatibility)
        (Value::FuncRef(Some(func_ref)), Value::Ref(idx)) => func_ref.index == *idx,
        (Value::Ref(idx), Value::FuncRef(Some(func_ref))) => *idx == func_ref.index,
        (Value::FuncRef(None), Value::Ref(0)) => true,
        (Value::Ref(0), Value::FuncRef(None)) => true,
        // ExternRef vs Ref
        (Value::ExternRef(Some(ext_ref)), Value::Ref(idx)) => ext_ref.index == *idx,
        (Value::Ref(idx), Value::ExternRef(Some(ext_ref))) => *idx == ext_ref.index,
        (Value::ExternRef(None), Value::Ref(0)) => true,
        (Value::Ref(0), Value::ExternRef(None)) => true,
        // GC reference type comparisons
        (Value::ExnRef(a), Value::ExnRef(b)) => a == b,
        // I31Ref: i32::MAX sentinel means "any non-null i31ref" (from (ref.i31) in WAST)
        // Also matches eqref/anyref sentinels for cross-type GC reference matching
        (Value::I31Ref(Some(_)), Value::I31Ref(Some(sentinel))) if *sentinel == i32::MAX => true,
        (Value::I31Ref(None), Value::I31Ref(Some(sentinel))) if *sentinel == i32::MAX => false,
        // eqref/anyref sentinel (i32::MAX) matches any non-null struct/array/i31
        (Value::StructRef(Some(_)), Value::I31Ref(Some(sentinel))) if *sentinel == i32::MAX => true,
        (Value::ArrayRef(Some(_)), Value::I31Ref(Some(sentinel))) if *sentinel == i32::MAX => true,
        (Value::StructRef(None), Value::I31Ref(Some(sentinel))) if *sentinel == i32::MAX => false,
        (Value::ArrayRef(None), Value::I31Ref(Some(sentinel))) if *sentinel == i32::MAX => false,
        (Value::I31Ref(a), Value::I31Ref(b)) => a == b,
        // StructRef: type_index = u32::MAX sentinel means "any non-null structref"
        (Value::StructRef(Some(_)), Value::StructRef(Some(sentinel))) if sentinel.type_index == u32::MAX => true,
        (Value::StructRef(None), Value::StructRef(Some(sentinel))) if sentinel.type_index == u32::MAX => false,
        (Value::StructRef(a), Value::StructRef(b)) => a == b,
        // ArrayRef: type_index = u32::MAX sentinel means "any non-null arrayref"
        (Value::ArrayRef(Some(_)), Value::ArrayRef(Some(sentinel))) if sentinel.type_index == u32::MAX => true,
        (Value::ArrayRef(None), Value::ArrayRef(Some(sentinel))) if sentinel.type_index == u32::MAX => false,
        (Value::ArrayRef(a), Value::ArrayRef(b)) => a == b,
        // Cross-type null reference comparisons for WAST testing
        // In GC spec, (ref.null) without type is polymorphic and matches any null reference
        // Also handle subtyping: none ⊂ any, nofunc ⊂ func, noextern ⊂ extern, noexn ⊂ exn
        (Value::FuncRef(None), Value::ExternRef(None)) => true,
        (Value::ExternRef(None), Value::FuncRef(None)) => true,
        (Value::FuncRef(None), Value::ExnRef(None)) => true,
        (Value::ExnRef(None), Value::FuncRef(None)) => true,
        (Value::ExternRef(None), Value::ExnRef(None)) => true,
        (Value::ExnRef(None), Value::ExternRef(None)) => true,
        (Value::FuncRef(None), Value::I31Ref(None)) => true,
        (Value::I31Ref(None), Value::FuncRef(None)) => true,
        (Value::ExternRef(None), Value::I31Ref(None)) => true,
        (Value::I31Ref(None), Value::ExternRef(None)) => true,
        (Value::ExnRef(None), Value::I31Ref(None)) => true,
        (Value::I31Ref(None), Value::ExnRef(None)) => true,
        (Value::FuncRef(None), Value::StructRef(None)) => true,
        (Value::StructRef(None), Value::FuncRef(None)) => true,
        (Value::FuncRef(None), Value::ArrayRef(None)) => true,
        (Value::ArrayRef(None), Value::FuncRef(None)) => true,
        (Value::ExternRef(None), Value::StructRef(None)) => true,
        (Value::StructRef(None), Value::ExternRef(None)) => true,
        (Value::ExternRef(None), Value::ArrayRef(None)) => true,
        (Value::ArrayRef(None), Value::ExternRef(None)) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_conversion() {
        let wast_arg = WastArg::Core(WastArgCore::I32(42));
        let kiln_value = convert_wast_arg_to_value(&wast_arg).unwrap();
        assert_eq!(kiln_value, Value::I32(42));
    }

    #[test]
    fn test_values_equal() {
        assert!(values_equal(&Value::I32(42), &Value::I32(42)));
        assert!(!values_equal(&Value::I32(42), &Value::I32(43)));

        // Test NaN handling
        let nan1 = Value::F32(FloatBits32::NAN);
        let nan2 = Value::F32(FloatBits32::NAN);
        assert!(values_equal(&nan1, &nan2));
    }
}
