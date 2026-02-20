//! Additional SIMD Operations for Complete WebAssembly 2.0 Implementation
//!
//! This module contains the remaining SIMD operation implementations that
//! complete the WebAssembly SIMD specification coverage.

#[cfg(not(feature = "std"))]
use alloc::format;

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
use wrt_foundation::values::{
    Value,
    V128,
};

/// Extract v128 bytes from a Value with validation
#[inline]
fn extract_v128(value: &Value) -> Result<[u8; 16]> {
    match value {
        Value::V128(v128) => Ok(v128.bytes),
        _ => Err(Error::runtime_execution_error(
            "Expected v128 value",
        )),
    }
}

// ================================================================================================
// Extended Multiplication Operations
// ================================================================================================

pub fn execute_i16x8_ext_mul_low_i8x16_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply low 8 i8 values to produce 8 i16 values
    for i in 0..8 {
        let a_val = a[i] as i8;
        let b_val = b[i] as i8;
        let product = (a_val as i16) * (b_val as i16);
        let product_bytes = product.to_le_bytes();
        result[i * 2..i * 2 + 2].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i16x8_ext_mul_high_i8x16_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply high 8 i8 values to produce 8 i16 values
    for i in 0..8 {
        let a_val = a[i + 8] as i8;
        let b_val = b[i + 8] as i8;
        let product = (a_val as i16) * (b_val as i16);
        let product_bytes = product.to_le_bytes();
        result[i * 2..i * 2 + 2].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i16x8_ext_mul_low_i8x16_u(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply low 8 u8 values to produce 8 i16 values
    for i in 0..8 {
        let a_val = a[i] as u8;
        let b_val = b[i] as u8;
        let product = (a_val as i16) * (b_val as i16);
        let product_bytes = product.to_le_bytes();
        result[i * 2..i * 2 + 2].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i16x8_ext_mul_high_i8x16_u(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply high 8 u8 values to produce 8 i16 values
    for i in 0..8 {
        let a_val = a[i + 8] as u8;
        let b_val = b[i + 8] as u8;
        let product = (a_val as i16) * (b_val as i16);
        let product_bytes = product.to_le_bytes();
        result[i * 2..i * 2 + 2].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i32x4_ext_mul_low_i16x8_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply low 4 i16 values to produce 4 i32 values
    for i in 0..4 {
        let a_bytes = &a[i * 2..i * 2 + 2];
        let b_bytes = &b[i * 2..i * 2 + 2];
        let a_val = i16::from_le_bytes([a_bytes[0], a_bytes[1]]);
        let b_val = i16::from_le_bytes([b_bytes[0], b_bytes[1]]);
        let product = (a_val as i32) * (b_val as i32);
        let product_bytes = product.to_le_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i32x4_ext_mul_high_i16x8_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply high 4 i16 values to produce 4 i32 values
    for i in 0..4 {
        let a_bytes = &a[(i + 4) * 2..(i + 4) * 2 + 2];
        let b_bytes = &b[(i + 4) * 2..(i + 4) * 2 + 2];
        let a_val = i16::from_le_bytes([a_bytes[0], a_bytes[1]]);
        let b_val = i16::from_le_bytes([b_bytes[0], b_bytes[1]]);
        let product = (a_val as i32) * (b_val as i32);
        let product_bytes = product.to_le_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i32x4_ext_mul_low_i16x8_u(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply low 4 u16 values to produce 4 i32 values
    for i in 0..4 {
        let a_bytes = &a[i * 2..i * 2 + 2];
        let b_bytes = &b[i * 2..i * 2 + 2];
        let a_val = u16::from_le_bytes([a_bytes[0], a_bytes[1]]);
        let b_val = u16::from_le_bytes([b_bytes[0], b_bytes[1]]);
        let product = (a_val as i32) * (b_val as i32);
        let product_bytes = product.to_le_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i32x4_ext_mul_high_i16x8_u(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply high 4 u16 values to produce 4 i32 values
    for i in 0..4 {
        let a_bytes = &a[(i + 4) * 2..(i + 4) * 2 + 2];
        let b_bytes = &b[(i + 4) * 2..(i + 4) * 2 + 2];
        let a_val = u16::from_le_bytes([a_bytes[0], a_bytes[1]]);
        let b_val = u16::from_le_bytes([b_bytes[0], b_bytes[1]]);
        let product = (a_val as i32) * (b_val as i32);
        let product_bytes = product.to_le_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i64x2_ext_mul_low_i32x4_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply low 2 i32 values to produce 2 i64 values
    for i in 0..2 {
        let a_bytes = &a[i * 4..i * 4 + 4];
        let b_bytes = &b[i * 4..i * 4 + 4];
        let a_val = i32::from_le_bytes([a_bytes[0], a_bytes[1], a_bytes[2], a_bytes[3]]);
        let b_val = i32::from_le_bytes([b_bytes[0], b_bytes[1], b_bytes[2], b_bytes[3]]);
        let product = (a_val as i64) * (b_val as i64);
        let product_bytes = product.to_le_bytes();
        result[i * 8..i * 8 + 8].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i64x2_ext_mul_high_i32x4_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply high 2 i32 values to produce 2 i64 values
    for i in 0..2 {
        let a_bytes = &a[(i + 2) * 4..(i + 2) * 4 + 4];
        let b_bytes = &b[(i + 2) * 4..(i + 2) * 4 + 4];
        let a_val = i32::from_le_bytes([a_bytes[0], a_bytes[1], a_bytes[2], a_bytes[3]]);
        let b_val = i32::from_le_bytes([b_bytes[0], b_bytes[1], b_bytes[2], b_bytes[3]]);
        let product = (a_val as i64) * (b_val as i64);
        let product_bytes = product.to_le_bytes();
        result[i * 8..i * 8 + 8].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i64x2_ext_mul_low_i32x4_u(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply low 2 u32 values to produce 2 i64 values
    for i in 0..2 {
        let a_bytes = &a[i * 4..i * 4 + 4];
        let b_bytes = &b[i * 4..i * 4 + 4];
        let a_val = u32::from_le_bytes([a_bytes[0], a_bytes[1], a_bytes[2], a_bytes[3]]);
        let b_val = u32::from_le_bytes([b_bytes[0], b_bytes[1], b_bytes[2], b_bytes[3]]);
        let product = (a_val as i64) * (b_val as i64);
        let product_bytes = product.to_le_bytes();
        result[i * 8..i * 8 + 8].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i64x2_ext_mul_high_i32x4_u(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Multiply high 2 u32 values to produce 2 i64 values
    for i in 0..2 {
        let a_bytes = &a[(i + 2) * 4..(i + 2) * 4 + 4];
        let b_bytes = &b[(i + 2) * 4..(i + 2) * 4 + 4];
        let a_val = u32::from_le_bytes([a_bytes[0], a_bytes[1], a_bytes[2], a_bytes[3]]);
        let b_val = u32::from_le_bytes([b_bytes[0], b_bytes[1], b_bytes[2], b_bytes[3]]);
        let product = (a_val as i64) * (b_val as i64);
        let product_bytes = product.to_le_bytes();
        result[i * 8..i * 8 + 8].copy_from_slice(&product_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

// ================================================================================================
// Pairwise Addition Operations
// ================================================================================================

pub fn execute_i16x8_ext_add_pairwise_i8x16_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let mut result = [0u8; 16];

    // Add pairs of i8 values to produce 8 i16 values
    for i in 0..8 {
        let a_val1 = a[i * 2] as i8;
        let a_val2 = a[i * 2 + 1] as i8;
        let sum = (a_val1 as i16) + (a_val2 as i16);
        let sum_bytes = sum.to_le_bytes();
        result[i * 2..i * 2 + 2].copy_from_slice(&sum_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i16x8_ext_add_pairwise_i8x16_u(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let mut result = [0u8; 16];

    // Add pairs of u8 values to produce 8 i16 values
    for i in 0..8 {
        let a_val1 = a[i * 2] as u8;
        let a_val2 = a[i * 2 + 1] as u8;
        let sum = (a_val1 as i16) + (a_val2 as i16);
        let sum_bytes = sum.to_le_bytes();
        result[i * 2..i * 2 + 2].copy_from_slice(&sum_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i32x4_ext_add_pairwise_i16x8_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let mut result = [0u8; 16];

    // Add pairs of i16 values to produce 4 i32 values
    for i in 0..4 {
        let a1_bytes = &a[i * 4..i * 4 + 2];
        let a2_bytes = &a[i * 4 + 2..i * 4 + 4];
        let a_val1 = i16::from_le_bytes([a1_bytes[0], a1_bytes[1]]);
        let a_val2 = i16::from_le_bytes([a2_bytes[0], a2_bytes[1]]);
        let sum = (a_val1 as i32) + (a_val2 as i32);
        let sum_bytes = sum.to_le_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&sum_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

pub fn execute_i32x4_ext_add_pairwise_i16x8_u(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let mut result = [0u8; 16];

    // Add pairs of u16 values to produce 4 i32 values
    for i in 0..4 {
        let a1_bytes = &a[i * 4..i * 4 + 2];
        let a2_bytes = &a[i * 4 + 2..i * 4 + 4];
        let a_val1 = u16::from_le_bytes([a1_bytes[0], a1_bytes[1]]);
        let a_val2 = u16::from_le_bytes([a2_bytes[0], a2_bytes[1]]);
        let sum = (a_val1 as i32) + (a_val2 as i32);
        let sum_bytes = sum.to_le_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&sum_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

// ================================================================================================
// Dot Product Operations
// ================================================================================================

pub fn execute_i32x4_dot_i16x8_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Compute dot product for pairs of i16 values to produce 4 i32 values
    for i in 0..4 {
        let a1_bytes = &a[i * 4..i * 4 + 2];
        let a2_bytes = &a[i * 4 + 2..i * 4 + 4];
        let b1_bytes = &b[i * 4..i * 4 + 2];
        let b2_bytes = &b[i * 4 + 2..i * 4 + 4];

        let a_val1 = i16::from_le_bytes([a1_bytes[0], a1_bytes[1]]);
        let a_val2 = i16::from_le_bytes([a2_bytes[0], a2_bytes[1]]);
        let b_val1 = i16::from_le_bytes([b1_bytes[0], b1_bytes[1]]);
        let b_val2 = i16::from_le_bytes([b2_bytes[0], b2_bytes[1]]);

        let dot = (a_val1 as i32) * (b_val1 as i32) + (a_val2 as i32) * (b_val2 as i32);
        let dot_bytes = dot.to_le_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&dot_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}

// ================================================================================================
// Q15 Multiplication Operations
// ================================================================================================

pub fn execute_i16x8_q15_mulr_sat_s(inputs: &[Value]) -> Result<Value> {
    let a = extract_v128(&inputs[0])?;
    let b = extract_v128(&inputs[1])?;
    let mut result = [0u8; 16];

    // Q15 multiplication with rounding and saturation
    for i in 0..8 {
        let a_bytes = &a[i * 2..i * 2 + 2];
        let b_bytes = &b[i * 2..i * 2 + 2];
        let a_val = i16::from_le_bytes([a_bytes[0], a_bytes[1]]);
        let b_val = i16::from_le_bytes([b_bytes[0], b_bytes[1]]);

        // Q15 multiplication: multiply and shift right by 15, with rounding
        let product = (a_val as i32) * (b_val as i32);
        let rounded = (product + 0x4000) >> 15; // Add 0x4000 for rounding, then shift

        // Saturate to i16 range
        let saturated = if rounded > 32767 {
            32767i16
        } else if rounded < -32768 {
            -32768i16
        } else {
            rounded as i16
        };

        let result_bytes = saturated.to_le_bytes();
        result[i * 2..i * 2 + 2].copy_from_slice(&result_bytes);
    }

    Ok(Value::V128(V128::new(result)))
}
