// WRT - wrt-math
// Module: Math Operations
// SW-REQ-ID: REQ_018 (Wasm numeric operations)
//
// Copyright (c) 2024 Your Name/Organization
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! This module implements the executable logic for WebAssembly numeric
//! instructions. It operates on primitive Rust types and `FloatBits` wrappers,
//! returning `Result` with `TrapCode` for Wasm-defined trapping behavior.

// Conditionally import std or core items for f32/f64
#[cfg(not(feature = "std"))]
use core::{
    f32,
    f64,
};
#[cfg(feature = "std")]
use std::{
    f32,
    f64,
};

use wrt_error::{
    codes::TrapCode,
    Error as WrtError,
    Result,
};

// Import necessary items from this crate's prelude and wrt-error
use crate::prelude::{
    FloatBits32,
    FloatBits64,
}; // Result, TrapCode, WrtError should come
   // via wrt_error directly or be aliased in
   // prelude

// Module-level constants for Wasm conversion boundaries
const I64_MAX_AS_F32: f32 = 9_223_372_036_854_775_808.0_f32;
const I64_MIN_AS_F32: f32 = -9_223_372_036_854_775_808.0_f32;

// --- Start of no_std math polyfills for trunc ---
#[cfg(not(feature = "std"))]
mod no_std_math_trunc {
    // Polyfill for f32::trunc
    pub(super) fn trunc_f32_polyfill(f_val: f32) -> f32 {
        if f_val.is_nan() || f_val.is_infinite() || f_val == 0.0 {
            // Use inherent methods on f32
            return f_val;
        }
        let bits = f_val.to_bits(); // Use inherent method on f32
        let sign_mask = 0x8000_0000u32;
        let exponent_mask = 0x7F80_0000u32;
        let mantissa_mask = 0x007F_FFFFu32;

        let sign = bits & sign_mask;
        let exponent_bits = (bits & exponent_mask) >> 23;

        if exponent_bits == 0xFF {
            // NaN or Inf already handled
            return f_val;
        }

        let biased_exponent = exponent_bits as i32;
        let exponent = biased_exponent - 127;

        if exponent < 0 {
            // Number is < 1.0 (and not 0), truncates to +/-0.0
            return f32::from_bits(sign); // Return +/-0.0
        }

        // Number of fractional bits to clear
        // Max is 23 (full mantissa if exponent is 0, means val < 2.0 and >= 1.0)
        // Min is 0 (if exponent >= 23, means val is an integer or larger)
        let fractional_bits_count = 23 - exponent;

        if fractional_bits_count <= 0 {
            // Already an integer or exponent too large
            return f_val;
        }

        // Create mask to clear fractional bits.
        // E.g., if fractional_bits_count = 5, we want to clear the 5 LSBs of mantissa.
        // Mask should be ...11100000. (1 << 5) - 1 = 0b11111. !0b11111 = ...11100000
        let clear_mask = !((1u32 << fractional_bits_count) - 1);
        let new_mantissa_bits = (bits & mantissa_mask) & clear_mask;

        f32::from_bits(sign | (exponent_bits << 23) | new_mantissa_bits)
    }

    // Polyfill for f64::trunc
    pub(super) fn trunc_f64_polyfill(d_val: f64) -> f64 {
        if d_val.is_nan() || d_val.is_infinite() || d_val == 0.0 {
            return d_val;
        }
        let bits = d_val.to_bits();
        let sign_mask = 0x8000_0000_0000_0000u64;
        let exponent_mask = 0x7FF0_0000_0000_0000u64;
        let mantissa_mask = 0x000F_FFFF_FFFF_FFFFu64;

        let sign = bits & sign_mask;
        let exponent_bits_u64 = (bits & exponent_mask) >> 52;

        if exponent_bits_u64 == 0x7FF {
            // NaN or Inf
            return d_val;
        }

        let biased_exponent = exponent_bits_u64 as i32;
        let exponent = biased_exponent - 1023;

        if exponent < 0 {
            // Number is < 1.0 (and not 0), truncates to +/-0.0
            return f64::from_bits(sign); // Return +/-0.0
        }

        let fractional_bits_count = 52 - exponent;

        if fractional_bits_count <= 0 {
            // Already an integer or exponent too large
            return d_val;
        }

        let clear_mask = !((1u64 << fractional_bits_count) - 1);
        let new_mantissa_bits = (bits & mantissa_mask) & clear_mask;

        f64::from_bits(sign | (exponent_bits_u64 << 52) | new_mantissa_bits)
    }
}
// --- End of no_std math polyfills for trunc ---

// --- Start of no_std math polyfills for rounding ---
#[cfg(not(feature = "std"))]
mod no_std_math_rounding {
    use super::no_std_math_trunc::{
        trunc_f32_polyfill,
        trunc_f64_polyfill,
    };

    // Polyfill for f32::floor
    pub(super) fn floor_f32_polyfill(f_val: f32) -> f32 {
        if f_val.is_nan() || f_val.is_infinite() || f_val == 0.0 {
            return f_val;
        }
        let t = trunc_f32_polyfill(f_val);
        if f_val >= 0.0 || t == f_val {
            // If positive or already an integer
            t
        } else {
            // Negative and not an integer
            t - 1.0
        }
    }

    // Polyfill for f32::ceil
    pub(super) fn ceil_f32_polyfill(f_val: f32) -> f32 {
        if f_val.is_nan() || f_val.is_infinite() || f_val == 0.0 {
            return f_val;
        }
        let t = trunc_f32_polyfill(f_val);
        if f_val <= 0.0 || t == f_val {
            // If negative or already an integer
            t
        } else {
            // Positive and not an integer
            t + 1.0
        }
    }

    // Polyfill for f32::round (ties to even)
    pub(super) fn round_ties_to_even_f32_polyfill(f_val: f32) -> f32 {
        if f_val.is_nan() || f_val.is_infinite() || f_val == 0.0 {
            return f_val;
        }

        // Basic idea: if fractional part is 0.5, round to the even integer.
        // Otherwise, round to the nearest integer.
        let fl = floor_f32_polyfill(f_val);
        let fr = f_val - fl; // Fractional part (0.0 to <1.0)

        if fr < 0.5 {
            return fl;
        }
        if fr > 0.5 {
            // This is ceil_f32_polyfill(f_val)
            return fl + 1.0;
        }

        // At this point, fr == 0.5. Round to even.
        // If fl is even, return fl. If fl is odd, return fl + 1.0 (which is ceil).
        let ce = fl + 1.0; // This is ceil_f32_polyfill(f_val)
                           // Check if fl is even: fl % 2.0 == 0.0
                           // An even number divided by 2 is an integer.
        let half_fl = fl / 2.0;
        if half_fl == trunc_f32_polyfill(half_fl) {
            // fl is even
            fl
        } else {
            // fl is odd
            ce
        }
    }

    // Polyfill for f64::floor
    pub(super) fn floor_f64_polyfill(d_val: f64) -> f64 {
        if d_val.is_nan() || d_val.is_infinite() || d_val == 0.0 {
            return d_val;
        }
        let t = trunc_f64_polyfill(d_val);
        if d_val >= 0.0 || t == d_val {
            t
        } else {
            t - 1.0
        }
    }

    // Polyfill for f64::ceil
    pub(super) fn ceil_f64_polyfill(d_val: f64) -> f64 {
        if d_val.is_nan() || d_val.is_infinite() || d_val == 0.0 {
            return d_val;
        }
        let t = trunc_f64_polyfill(d_val);
        if d_val <= 0.0 || t == d_val {
            t
        } else {
            t + 1.0
        }
    }

    // Polyfill for f64::round (ties to even)
    pub(super) fn round_ties_to_even_f64_polyfill(d_val: f64) -> f64 {
        if d_val.is_nan() || d_val.is_infinite() || d_val == 0.0 {
            return d_val;
        }

        let fl = floor_f64_polyfill(d_val);
        let fr = d_val - fl;

        if fr < 0.5 {
            return fl;
        }
        if fr > 0.5 {
            // This is ceil_f64_polyfill(d_val)
            return fl + 1.0;
        }

        // fr == 0.5, round to even
        let ce = fl + 1.0; // This is ceil_f64_polyfill(d_val)
        let half_fl = fl / 2.0;
        if half_fl == trunc_f64_polyfill(half_fl) {
            // fl is even
            fl
        } else {
            // fl is odd
            ce
        }
    }
}
// --- End of no_std math polyfills for rounding ---

// --- Start of no_std math polyfills for sqrt ---
#[cfg(not(feature = "std"))]
mod no_std_math_sqrt {
    use crate::float_bits::{
        FloatBits32,
        FloatBits64,
    }; // Keep for NAN const access

    // Polyfill for f32::sqrt using Newton-Raphson method
    pub(super) fn sqrt_f32_polyfill(f_val: f32) -> f32 {
        if f_val.is_nan() {
            // Wasm spec: "if z is a NaN, then return a canonical NaN".
            // Our FloatBits32::NAN.value() should be a canonical NaN.
            return FloatBits32::NAN.value();
        }
        if f_val < 0.0 {
            // sqrt of negative number (excluding -0.0) is NaN.
            return FloatBits32::NAN.value();
        }
        if f_val == 0.0 {
            // Handles +0.0 and -0.0. sqrt(+0)=+0, sqrt(-0)=-0.
            return f_val;
        }
        if f_val.is_infinite() && f_val.is_sign_positive() {
            // sqrt(+Inf) = +Inf
            return f_val;
        }
        // At this point, f_val is a positive, finite, non-zero number.

        if f_val == 1.0 {
            return 1.0;
        } // Exact case, avoids iterations.

        // Initial guess for Newton-Raphson.
        // A simple, generally safe guess:
        let mut y: f32 = if f_val > 1.0 { f_val * 0.5 } else { 1.0 };
        if y == 0.0 {
            // Should only happen if f_val was extremely small, leading to y=0
            y = f32::MIN_POSITIVE; // Use a tiny positive number to start if
                                   // f_val * 0.5 underflowed
        }

        // Newton-Raphson iterations: y_new = (y_old + x / y_old) / 2.0
        // Perform a fixed number of iterations. 8 for f32.
        let mut prev_y;
        for _ in 0..8 {
            // 8 iterations chosen for f32
            prev_y = y;
            if y == 0.0 {
                // Avoid division by zero if y somehow becomes 0
                // This case should ideally not be reached if f_val > 0 and initial y > 0
                // This implies f_val is extremely small, or an issue occurred.
                // Result for very small positive f_val should be very small positive.
                return 0.0f32.copysign(f_val); // Or handle as an underflow to
                                               // 0, preserving sign.
            }
            y = (y + f_val / y) * 0.5;
            if y == prev_y {
                // Converged
                break;
            }
        }
        y
    }

    // Polyfill for f64::sqrt using Newton-Raphson method
    pub(super) fn sqrt_f64_polyfill(d_val: f64) -> f64 {
        if d_val.is_nan() {
            return FloatBits64::NAN.value();
        }
        if d_val < 0.0 {
            return FloatBits64::NAN.value();
        }
        if d_val == 0.0 {
            return d_val;
        }
        if d_val.is_infinite() && d_val.is_sign_positive() {
            return d_val;
        }
        // At this point, d_val is a positive, finite, non-zero number.

        if d_val == 1.0 {
            return 1.0;
        } // Exact case

        // Initial guess
        let mut y: f64 = if d_val > 1.0 { d_val * 0.5 } else { 1.0 };
        if y == 0.0 {
            // Should only happen if d_val was extremely small
            y = f64::MIN_POSITIVE;
        }

        // Newton-Raphson iterations. 12 for f64.
        let mut prev_y;
        for _ in 0..12 {
            // 12 iterations chosen for f64
            prev_y = y;
            if y == 0.0 {
                // Avoid division by zero
                return 0.0f64.copysign(d_val);
            }
            y = (y + d_val / y) * 0.5;
            if y == prev_y {
                // Converged
                break;
            }
        }
        y
    }
}
// --- End of no_std math polyfills for sqrt ---

// --- Compatibility wrappers for float functions (std vs no_std polyfill) ---

#[inline]
fn f32_trunc_compat(f: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        f.trunc()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_trunc::trunc_f32_polyfill(f)
    }
}

#[inline]
fn f32_ceil_compat(f: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        f.ceil()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_rounding::ceil_f32_polyfill(f)
    }
}

#[inline]
fn f32_floor_compat(f: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        f.floor()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_rounding::floor_f32_polyfill(f)
    }
}

#[inline]
fn f32_round_ties_to_even_compat(f: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        // Rust's f32::round behavior is round half to even.
        f.round()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_rounding::round_ties_to_even_f32_polyfill(f)
    }
}

#[inline]
fn f32_sqrt_compat(f: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        f.sqrt()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_sqrt::sqrt_f32_polyfill(f)
    }
}

// Conditional wrapper for f64::trunc
#[inline]
fn f64_trunc_compat(d: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        d.trunc()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_trunc::trunc_f64_polyfill(d)
    }
}

#[inline]
fn f64_ceil_compat(d: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        d.ceil()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_rounding::ceil_f64_polyfill(d)
    }
}

#[inline]
fn f64_floor_compat(d: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        d.floor()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_rounding::floor_f64_polyfill(d)
    }
}

#[inline]
fn f64_round_ties_to_even_compat(d: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        // Rust's f64::round behavior is round half to even.
        d.round()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_rounding::round_ties_to_even_f64_polyfill(d)
    }
}

#[inline]
fn f64_sqrt_compat(d: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        d.sqrt()
    }
    #[cfg(not(feature = "std"))]
    {
        no_std_math_sqrt::sqrt_f64_polyfill(d)
    }
}

// Manual signum for no_std, or std version
// This was dead code in the original file, but kept for completeness if signum
// is needed.
#[cfg(feature = "std")]
#[allow(dead_code)]
fn f64_signum_compat(x: f64) -> f64 {
    x.signum()
}

#[cfg(not(feature = "std"))]
#[allow(dead_code)]
fn f64_signum_compat(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    } // Or f64::NAN, Wasm usually propagates NaN payload
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    } // Preserve sign of 0.0 / -0.0
}

// --- Helper for Traps ---

/// Helper function to create a WRT trap error.
#[inline]
fn trap(code: TrapCode) -> WrtError {
    // Using specific category and code for traps
    code.into() // Relies on the From<TrapCode> for WrtError impl
}

// --- I32 Operations ---

/// i32.add: Add two i32 values.
/// Performs wrapping addition (modulo 2^32). Does not trap on overflow.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_add(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(lhs.wrapping_add(rhs))
}

/// i32.sub: Subtract two i32 values.
/// Performs wrapping subtraction (modulo 2^32). Does not trap on overflow.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_sub(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(lhs.wrapping_sub(rhs))
}

/// i32.mul: Multiply two i32 values.
/// Performs wrapping multiplication (modulo 2^32). Does not trap on overflow.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_mul(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(lhs.wrapping_mul(rhs))
}

/// `i32.div_s`: Signed i32 division.
/// Traps on division by zero.
/// Traps on overflow (`i32::MIN` / -1).
///
/// # Errors
///
/// - `Err(TrapCode::IntegerDivideByZero)` if `rhs` is 0.
/// - `Err(TrapCode::IntegerOverflow)` if `lhs` is `i32::MIN` and `rhs` is -1.
#[inline]
pub fn i32_div_s(lhs: i32, rhs: i32) -> Result<i32> {
    if rhs == 0 {
        return Err(trap(TrapCode::IntegerDivideByZero));
    }
    // Specific Wasm trap condition: min_signed / -1 overflows.
    // Rust's `wrapping_div` for i32::MIN / -1 results in i32::MIN,
    // but Wasm requires a trap here.
    if lhs == i32::MIN && rhs == -1 {
        return Err(trap(TrapCode::IntegerOverflow));
    }
    // Standard Rust division truncates towards zero, as per Wasm.
    // wrapping_div handles i32::MIN / -1 by returning i32::MIN,
    // which we've already trapped for above. For other cases, it's fine.
    Ok(lhs.wrapping_div(rhs))
}

/// `i32.div_u`: Unsigned i32 division.
/// Traps on division by zero.
///
/// # Errors
///
/// Returns `Err(TrapCode::IntegerDivideByZero)` if `rhs` is 0.
#[inline]
pub fn i32_div_u(lhs: u32, rhs: u32) -> Result<u32> {
    if rhs == 0 {
        return Err(trap(TrapCode::IntegerDivideByZero));
    }
    // Standard Rust unsigned division, as per Wasm.
    Ok(lhs / rhs)
}

/// `i32.rem_s`: Signed i32 remainder.
/// Traps on division by zero.
/// The result has the sign of the dividend.
///
/// # Errors
///
/// Returns `Err(TrapCode::IntegerDivideByZero)` if `rhs` is 0.
#[inline]
pub fn i32_rem_s(lhs: i32, rhs: i32) -> Result<i32> {
    if rhs == 0 {
        return Err(trap(TrapCode::IntegerDivideByZero));
    }
    // Wasm spec for `i32.rem_s` with `i32::MIN` and `-1`: result is 0.
    // Rust's `wrapping_rem(i32::MIN, -1)` is 0.
    Ok(lhs.wrapping_rem(rhs))
}

/// `i32.rem_u`: Unsigned i32 remainder.
/// Traps on division by zero.
///
/// # Errors
///
/// Returns `Err(TrapCode::IntegerDivideByZero)` if `rhs` is 0.
#[inline]
pub fn i32_rem_u(lhs: u32, rhs: u32) -> Result<u32> {
    if rhs == 0 {
        return Err(trap(TrapCode::IntegerDivideByZero));
    }
    Ok(lhs % rhs)
}

/// i32.and: Bitwise AND of two i32 values.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_and(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(lhs & rhs)
}

/// i32.or: Bitwise OR of two i32 values.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_or(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(lhs | rhs)
}

/// i32.xor: Bitwise XOR of two i32 values.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_xor(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(lhs ^ rhs)
}

/// i32.shl: Shift left (logical) i32 value.
/// Shift amount `k` is taken modulo 32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_shl(lhs: i32, rhs: i32) -> Result<i32> {
    // Wasm: `k` is `rhs as u32`. Shift amount is `k mod 32`.
    // Rust's `wrapping_shl` effectively does this by masking `rhs` to relevant
    // bits.
    Ok(lhs.wrapping_shl(rhs as u32))
}

/// `i32.shr_s`: Shift right (arithmetic) signed i32 value.
/// Shift amount `k` is taken modulo 32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_shr_s(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(lhs.wrapping_shr(rhs as u32))
}

/// `i32.shr_u`: Shift right (logical) unsigned i32 value (acting on i32 bits).
/// Shift amount `k` is taken modulo 32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_shr_u(lhs: i32, rhs: i32) -> Result<i32> {
    // Perform as u32 to ensure logical shift, then cast back to i32.
    Ok((lhs as u32).wrapping_shr(rhs as u32) as i32)
}

/// i32.rotl: Rotate left i32 value.
/// Rotate amount `k` is taken modulo 32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_rotl(lhs: i32, rhs: i32) -> Result<i32> {
    // `rhs as u32` correctly handles the effective modulo 32 for rotate_left.
    Ok(lhs.rotate_left(rhs as u32))
}

/// i32.rotr: Rotate right i32 value.
/// Rotate amount `k` is taken modulo 32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_rotr(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(lhs.rotate_right(rhs as u32))
}

/// i32.clz: Count leading zeros in an i32 value.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_clz(val: i32) -> Result<i32> {
    Ok(val.leading_zeros() as i32)
}

/// i32.ctz: Count trailing zeros in an i32 value.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_ctz(val: i32) -> Result<i32> {
    Ok(val.trailing_zeros() as i32)
}

/// i32.popcnt: Count population (number of set bits) in an i32 value.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_popcnt(val: i32) -> Result<i32> {
    Ok(val.count_ones() as i32)
}

/// i32.eqz: Check if i32 value is zero. Returns 1 if zero, 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_eqz(val: i32) -> Result<i32> {
    Ok(i32::from(val == 0))
}

// --- I32 Additional Arithmetic Operations ---

/// i32.neg: Two's complement negation.
/// Equivalent to 0 - value.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_neg(val: i32) -> Result<i32> {
    Ok(val.wrapping_neg())
}

/// i32.abs: Absolute value.
/// Note: abs(i32::MIN) wraps to i32::MIN in two's complement.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_abs(val: i32) -> Result<i32> {
    Ok(val.wrapping_abs())
}

// --- I32 Comparison Operations ---

/// i32.eq: Check if two i32 values are equal. Returns 1 if equal, 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_eq(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(i32::from(lhs == rhs))
}

/// i32.ne: Check if two i32 values are not equal. Returns 1 if not equal, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_ne(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(i32::from(lhs != rhs))
}

/// i32.lt_s: Signed less than comparison. Returns 1 if lhs < rhs, 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_lt_s(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(i32::from(lhs < rhs))
}

/// i32.lt_u: Unsigned less than comparison. Returns 1 if lhs < rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_lt_u(lhs: u32, rhs: u32) -> Result<i32> {
    Ok(i32::from(lhs < rhs))
}

/// i32.gt_s: Signed greater than comparison. Returns 1 if lhs > rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_gt_s(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(i32::from(lhs > rhs))
}

/// i32.gt_u: Unsigned greater than comparison. Returns 1 if lhs > rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_gt_u(lhs: u32, rhs: u32) -> Result<i32> {
    Ok(i32::from(lhs > rhs))
}

/// i32.le_s: Signed less than or equal comparison. Returns 1 if lhs <= rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_le_s(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(i32::from(lhs <= rhs))
}

/// i32.le_u: Unsigned less than or equal comparison. Returns 1 if lhs <= rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_le_u(lhs: u32, rhs: u32) -> Result<i32> {
    Ok(i32::from(lhs <= rhs))
}

/// i32.ge_s: Signed greater than or equal comparison. Returns 1 if lhs >= rhs,
/// 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_ge_s(lhs: i32, rhs: i32) -> Result<i32> {
    Ok(i32::from(lhs >= rhs))
}

/// i32.ge_u: Unsigned greater than or equal comparison. Returns 1 if lhs >=
/// rhs, 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_ge_u(lhs: u32, rhs: u32) -> Result<i32> {
    Ok(i32::from(lhs >= rhs))
}

// --- I64 Operations ---

/// i64.add: Add two i64 values.
/// Performs wrapping addition (modulo 2^64). Does not trap on overflow.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_add(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs.wrapping_add(rhs))
}

/// i64.sub: Subtract two i64 values.
/// Performs wrapping subtraction (modulo 2^64). Does not trap on overflow.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_sub(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs.wrapping_sub(rhs))
}

/// i64.mul: Multiply two i64 values.
/// Performs wrapping multiplication (modulo 2^64). Does not trap on overflow.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_mul(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs.wrapping_mul(rhs))
}

/// `i64.div_s`: Signed i64 division.
/// Traps on division by zero or signed overflow (`i64::MIN` / -1).
///
/// # Errors
/// - `Err(TrapCode::IntegerDivideByZero)` if `rhs` is 0.
/// - `Err(TrapCode::IntegerOverflow)` if `lhs` is `i64::MIN` and `rhs` is -1.
#[inline]
pub fn i64_div_s(lhs: i64, rhs: i64) -> Result<i64> {
    if rhs == 0 {
        return Err(trap(TrapCode::IntegerDivideByZero));
    }
    if lhs == i64::MIN && rhs == -1 {
        return Err(trap(TrapCode::IntegerOverflow));
    }
    // Wasm division truncates. Rust's wrapping_div does this.
    Ok(lhs.wrapping_div(rhs))
}

/// `i64.div_u`: Unsigned i64 division.
/// Traps on division by zero.
///
/// # Errors
///
/// Returns `Err(TrapCode::IntegerDivideByZero)` if `rhs` is 0.
#[inline]
pub fn i64_div_u(lhs: u64, rhs: u64) -> Result<u64> {
    if rhs == 0 {
        return Err(trap(TrapCode::IntegerDivideByZero));
    }
    Ok(lhs / rhs)
}

/// `i64.rem_s`: Signed i64 remainder.
/// Traps on division by zero.
/// The result has the sign of the dividend.
///
/// # Errors
///
/// Returns `Err(TrapCode::IntegerDivideByZero)` if `rhs` is 0.
#[inline]
pub fn i64_rem_s(lhs: i64, rhs: i64) -> Result<i64> {
    if rhs == 0 {
        return Err(trap(TrapCode::IntegerDivideByZero));
    }
    // `i64::MIN % -1` result is 0 in Rust, which is correct by Wasm spec.
    Ok(lhs.wrapping_rem(rhs))
}

/// `i64.rem_u`: Unsigned i64 remainder.
/// Traps on division by zero.
///
/// # Errors
///
/// Returns `Err(TrapCode::IntegerDivideByZero)` if `rhs` is 0.
#[inline]
pub fn i64_rem_u(lhs: u64, rhs: u64) -> Result<u64> {
    if rhs == 0 {
        return Err(trap(TrapCode::IntegerDivideByZero));
    }
    Ok(lhs % rhs)
}

/// i64.and: Bitwise AND of two i64 values.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_and(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs & rhs)
}

/// i64.or: Bitwise OR of two i64 values.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_or(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs | rhs)
}

/// i64.xor: Bitwise XOR of two i64 values.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_xor(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs ^ rhs)
}

/// i64.shl: Shift left (logical) i64 value.
/// Shift amount `k` is taken modulo 64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_shl(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs.wrapping_shl(rhs as u32))
}

/// `i64.shr_s`: Shift right (arithmetic) signed i64 value.
/// Shift amount `k` is taken modulo 64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_shr_s(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs.wrapping_shr(rhs as u32))
}

/// `i64.shr_u`: Shift right (logical) unsigned i64 value (acting on i64 bits).
/// Shift amount `k` is taken modulo 64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_shr_u(lhs: i64, rhs: i64) -> Result<i64> {
    Ok((lhs as u64).wrapping_shr(rhs as u32) as i64)
}

/// i64.rotl: Rotate left i64 value.
/// Rotate amount `k` is taken modulo 64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_rotl(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs.rotate_left(rhs as u32))
}

/// i64.rotr: Rotate right i64 value.
/// Rotate amount `k` is taken modulo 64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_rotr(lhs: i64, rhs: i64) -> Result<i64> {
    Ok(lhs.rotate_right(rhs as u32))
}

/// i64.clz: Count leading zeros in an i64 value.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_clz(val: i64) -> Result<i64> {
    Ok(i64::from(val.leading_zeros()))
}

/// i64.ctz: Count trailing zeros in an i64 value.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_ctz(val: i64) -> Result<i64> {
    Ok(i64::from(val.trailing_zeros()))
}

/// i64.popcnt: Count population (number of set bits) in an i64 value.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_popcnt(val: i64) -> Result<i64> {
    Ok(i64::from(val.count_ones()))
}

/// i64.eqz: Check if i64 value is zero. Returns 1 if zero, 0 otherwise.
/// (Wasm `i64.eqz` returns `i32`)
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_eqz(val: i64) -> Result<i32> {
    Ok(i32::from(val == 0))
}

// --- I64 Additional Arithmetic Operations ---

/// i64.neg: Two's complement negation.
/// Equivalent to 0 - value.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_neg(val: i64) -> Result<i64> {
    Ok(val.wrapping_neg())
}

/// i64.abs: Absolute value.
/// Note: abs(i64::MIN) wraps to i64::MIN in two's complement.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_abs(val: i64) -> Result<i64> {
    Ok(val.wrapping_abs())
}

// --- I64 Comparison Operations ---

/// i64.eq: Check if two i64 values are equal. Returns 1 if equal, 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_eq(lhs: i64, rhs: i64) -> Result<i32> {
    Ok(i32::from(lhs == rhs))
}

/// i64.ne: Check if two i64 values are not equal. Returns 1 if not equal, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_ne(lhs: i64, rhs: i64) -> Result<i32> {
    Ok(i32::from(lhs != rhs))
}

/// i64.lt_s: Signed less than comparison. Returns 1 if lhs < rhs, 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_lt_s(lhs: i64, rhs: i64) -> Result<i32> {
    Ok(i32::from(lhs < rhs))
}

/// i64.lt_u: Unsigned less than comparison. Returns 1 if lhs < rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_lt_u(lhs: u64, rhs: u64) -> Result<i32> {
    Ok(i32::from(lhs < rhs))
}

/// i64.gt_s: Signed greater than comparison. Returns 1 if lhs > rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_gt_s(lhs: i64, rhs: i64) -> Result<i32> {
    Ok(i32::from(lhs > rhs))
}

/// i64.gt_u: Unsigned greater than comparison. Returns 1 if lhs > rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_gt_u(lhs: u64, rhs: u64) -> Result<i32> {
    Ok(i32::from(lhs > rhs))
}

/// i64.le_s: Signed less than or equal comparison. Returns 1 if lhs <= rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_le_s(lhs: i64, rhs: i64) -> Result<i32> {
    Ok(i32::from(lhs <= rhs))
}

/// i64.le_u: Unsigned less than or equal comparison. Returns 1 if lhs <= rhs, 0
/// otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_le_u(lhs: u64, rhs: u64) -> Result<i32> {
    Ok(i32::from(lhs <= rhs))
}

/// i64.ge_s: Signed greater than or equal comparison. Returns 1 if lhs >= rhs,
/// 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_ge_s(lhs: i64, rhs: i64) -> Result<i32> {
    Ok(i32::from(lhs >= rhs))
}

/// i64.ge_u: Unsigned greater than or equal comparison. Returns 1 if lhs >=
/// rhs, 0 otherwise.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_ge_u(lhs: u64, rhs: u64) -> Result<i32> {
    Ok(i32::from(lhs >= rhs))
}

// --- F32 Operations ---

/// f32.add: Add two f32 values.
/// Follows IEEE 754 arithmetic for `NaN` propagation, infinities, etc.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn f32_add(lhs: FloatBits32, rhs: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(lhs.value() + rhs.value()))
}

/// f32.sub: Subtract two f32 values.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn f32_sub(lhs: FloatBits32, rhs: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(lhs.value() - rhs.value()))
}

/// f32.mul: Multiply two f32 values.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn f32_mul(lhs: FloatBits32, rhs: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(lhs.value() * rhs.value()))
}

/// f32.div: Divide two f32 values.
/// Division by zero yields +/-infinity or `NaN` as per IEEE 754.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn f32_div(lhs: FloatBits32, rhs: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(lhs.value() / rhs.value()))
}

/// f32.abs: Absolute value of an f32.
/// Wasm `abs` clears the sign bit. If the input is canonical `NaN`, result is
/// canonical `NaN`. If input is sNaN, result is canonical `NaN`.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_abs(val: FloatBits32) -> Result<FloatBits32> {
    let f = val.value();
    if f.is_nan() {
        // Wasm spec: "if z is a NaN, then return a canonical NaN"
        Ok(FloatBits32::NAN)
    } else {
        // Clears the sign bit. For non-NaNs, f.abs() does this.
        // Rust's f32::abs preserves NaN payload but clears sign bit.
        // Wasm expects canonical NaN on NaN input, which is handled above.
        Ok(FloatBits32::from_float(f.abs()))
    }
}

/// f32.neg: Negate an f32 value.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_neg(val: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(-val.value()))
}

/// f32.copysign: Copy sign from one f32 to another.
/// result has magnitude of lhs, sign of rhs.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_copysign(lhs: FloatBits32, rhs: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(lhs.value().copysign(rhs.value())))
}

/// f32.ceil: Ceiling of an f32 value.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_ceil(val: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(f32_ceil_compat(val.value())))
}

/// f32.floor: Floor of an f32 value.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_floor(val: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(f32_floor_compat(val.value())))
}

/// f32.trunc: Truncate f32 value (to integer towards zero).
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_trunc(val: FloatBits32) -> Result<FloatBits32> {
    // Note: f32_trunc_compat returns f32, we need to ensure bit patterns are
    // preserved if that's critical. .to_bits().from_bits() ensures this.
    Ok(FloatBits32::from_bits(
        f32_trunc_compat(val.value()).to_bits(),
    ))
}

/// f32.nearest: Round f32 to nearest integer (ties to even).
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_nearest(val: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(f32_round_ties_to_even_compat(
        val.value(),
    )))
}

/// f32.sqrt: Square root of an f32 value.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_sqrt(val: FloatBits32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(f32_sqrt_compat(val.value())))
}

/// f32.min: Minimum of two f32 values (WASM semantics).
/// If either operand is `NaN`, result is canonical `NaN`.
/// -0.0 is less than +0.0.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_min(lhs: FloatBits32, rhs: FloatBits32) -> Result<FloatBits32> {
    let l = lhs.value();
    let r = rhs.value();

    if l.is_nan() || r.is_nan() {
        Ok(FloatBits32::NAN)
    } else if l == r && l == 0.0 {
        // Special handling for +0.0 and -0.0
        // Wasm: min(-0.0, +0.0) is -0.0. min(+0.0, -0.0) is -0.0.
        // If l is -0.0 (negative sign bit), it's smaller or equal.
        if l.is_sign_negative() {
            Ok(lhs)
        } else {
            Ok(rhs)
        } // If l is +0.0, r must be -0.0 or +0.0
    } else {
        // Standard comparison for non-NaN, non-zero cases.
        // Rust's f32::min behaves correctly for Wasm's non-NaN requirements.
        Ok(FloatBits32::from_float(l.min(r)))
    }
}

/// f32.max: Maximum of two f32 values (WASM semantics).
/// If either operand is `NaN`, result is canonical `NaN`.
/// +0.0 is greater than -0.0.
///
/// # Errors
///
/// This function does not currently return an error.
#[inline]
pub fn wasm_f32_max(lhs: FloatBits32, rhs: FloatBits32) -> Result<FloatBits32> {
    let l = lhs.value();
    let r = rhs.value();

    if l.is_nan() || r.is_nan() {
        Ok(FloatBits32::NAN)
    } else if l == r && l == 0.0 {
        // Special handling for +0.0 and -0.0
        // Wasm: max(-0.0, +0.0) is +0.0. max(+0.0, -0.0) is +0.0.
        // If l is +0.0 (positive sign bit), it's greater or equal.
        if l.is_sign_positive() {
            Ok(lhs)
        } else {
            Ok(rhs)
        } // If l is -0.0, r must be +0.0 or -0.0
    } else {
        // Rust's f32::max behaves correctly for Wasm's non-NaN requirements.
        Ok(FloatBits32::from_float(l.max(r)))
    }
}

// --- F64 Operations ---

/// f64.add: Add two f64 values.
#[inline]
pub fn f64_add(lhs: FloatBits64, rhs: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(lhs.value() + rhs.value()))
}

/// f64.sub: Subtract two f64 values.
#[inline]
pub fn f64_sub(lhs: FloatBits64, rhs: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(lhs.value() - rhs.value()))
}

/// f64.mul: Multiply two f64 values.
#[inline]
pub fn f64_mul(lhs: FloatBits64, rhs: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(lhs.value() * rhs.value()))
}

/// f64.div: Divide two f64 values.
#[inline]
pub fn f64_div(lhs: FloatBits64, rhs: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(lhs.value() / rhs.value()))
}

/// f64.abs: Absolute value of an f64.
/// Wasm `abs` clears the sign bit. If the input is canonical `NaN`, result is
/// canonical `NaN`.
#[inline]
pub fn wasm_f64_abs(val: FloatBits64) -> Result<FloatBits64> {
    let d = val.value();
    if d.is_nan() {
        Ok(FloatBits64::NAN)
    } else {
        Ok(FloatBits64::from_float(d.abs()))
    }
}

/// f64.neg: Negate an f64 value.
#[inline]
pub fn wasm_f64_neg(val: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(-val.value()))
}

/// f64.copysign: Copy sign from one f64 to another.
#[inline]
pub fn wasm_f64_copysign(lhs: FloatBits64, rhs: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(lhs.value().copysign(rhs.value())))
}

/// f64.ceil: Ceiling of an f64 value.
#[inline]
pub fn wasm_f64_ceil(val: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(f64_ceil_compat(val.value())))
}

/// f64.floor: Floor of an f64 value.
#[inline]
pub fn wasm_f64_floor(val: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(f64_floor_compat(val.value())))
}

/// f64.trunc: Truncate f64 value (to integer towards zero).
#[inline]
pub fn wasm_f64_trunc(val: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_bits(
        f64_trunc_compat(val.value()).to_bits(),
    ))
}

/// f64.nearest: Round to nearest integer, ties to even.
/// Follows IEEE 754-2008 `roundToIntegralTiesToEven`.
#[inline]
pub fn wasm_f64_nearest(val: FloatBits64) -> Result<FloatBits64> {
    let x = val.value();
    // Wasm spec: NaN -> canonical NaN; +/-Inf -> +/-Inf; +/-0 -> +/-0
    if x.is_nan() {
        return Ok(FloatBits64::NAN);
    }
    if x.is_infinite() || x == 0.0 {
        return Ok(val);
    }

    Ok(FloatBits64::from_float(f64_round_ties_to_even_compat(x)))
}

/// f64.sqrt: Square root of an f64 value.
#[inline]
pub fn wasm_f64_sqrt(val: FloatBits64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(f64_sqrt_compat(val.value())))
}

/// f64.min: Minimum of two f64 values (WASM semantics).
#[inline]
pub fn wasm_f64_min(lhs: FloatBits64, rhs: FloatBits64) -> Result<FloatBits64> {
    let l = lhs.value();
    let r = rhs.value();
    if l.is_nan() || r.is_nan() {
        Ok(FloatBits64::NAN)
    } else if l == r && l == 0.0 {
        if l.is_sign_negative() {
            Ok(lhs)
        } else {
            Ok(rhs)
        }
    } else {
        Ok(FloatBits64::from_float(l.min(r)))
    }
}

/// f64.max: Maximum of two f64 values (WASM semantics).
#[inline]
pub fn wasm_f64_max(lhs: FloatBits64, rhs: FloatBits64) -> Result<FloatBits64> {
    let l = lhs.value();
    let r = rhs.value();
    if l.is_nan() || r.is_nan() {
        Ok(FloatBits64::NAN)
    } else if l == r && l == 0.0 {
        if l.is_sign_positive() {
            Ok(lhs)
        } else {
            Ok(rhs)
        }
    } else {
        Ok(FloatBits64::from_float(l.max(r)))
    }
}

// --- Float Comparisons (all return i32: 0 for false, 1 for true) ---

// F32 Comparisons
/// WebAssembly f32.eq operation.
#[inline]
pub fn f32_eq(lhs: FloatBits32, rhs: FloatBits32) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    // Wasm: NaN == X is false. NaN == NaN is false.
    // Standard Rust `l == r` handles this correctly for non-NaNs.
    // If either is NaN, `l == r` is false.
    Ok(i32::from(l == r))
}

/// WebAssembly f32.ne operation.
#[inline]
pub fn f32_ne(lhs: FloatBits32, rhs: FloatBits32) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    // Wasm: NaN != X is true. NaN != NaN is true.
    // Standard Rust `l != r` handles this correctly.
    Ok(i32::from(l != r))
}

/// WebAssembly f32.lt operation.
#[inline]
pub fn f32_lt(lhs: FloatBits32, rhs: FloatBits32) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    // Wasm: if either is NaN, result is false.
    // Rust `l < r` is false if either is NaN.
    Ok(i32::from(l < r))
}

/// WebAssembly f32.gt operation.
#[inline]
pub fn f32_gt(lhs: FloatBits32, rhs: FloatBits32) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l > r))
}

/// WebAssembly f32.le operation.
#[inline]
pub fn f32_le(lhs: FloatBits32, rhs: FloatBits32) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l <= r))
}

/// WebAssembly f32.ge operation.
#[inline]
pub fn f32_ge(lhs: FloatBits32, rhs: FloatBits32) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l >= r))
}

// F64 Comparisons
/// WebAssembly f64.eq operation.
#[inline]
pub fn f64_eq(lhs: FloatBits64, rhs: FloatBits64) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l == r))
}

/// WebAssembly f64.ne operation.
#[inline]
pub fn f64_ne(lhs: FloatBits64, rhs: FloatBits64) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l != r))
}

/// WebAssembly f64.lt operation.
#[inline]
pub fn f64_lt(lhs: FloatBits64, rhs: FloatBits64) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l < r))
}

/// WebAssembly f64.gt operation.
#[inline]
pub fn f64_gt(lhs: FloatBits64, rhs: FloatBits64) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l > r))
}

/// WebAssembly f64.le operation.
#[inline]
pub fn f64_le(lhs: FloatBits64, rhs: FloatBits64) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l <= r))
}

/// WebAssembly f64.ge operation.
#[inline]
pub fn f64_ge(lhs: FloatBits64, rhs: FloatBits64) -> Result<i32> {
    let l = lhs.value();
    let r = rhs.value();
    Ok(i32::from(l >= r))
}

// --- Saturating Float to Integer Conversions ---
// These return the integer directly, not a Result<T>, as saturation implies no
// trap.

/// Converts an f32 to i32 using saturating truncation (Wasm semantics
/// `i32.trunc_sat_f32_s`). NaN -> 0. +/-Inf -> respective min/max. Out of range
/// -> respective min/max.
#[inline]
#[must_use = "Saturating conversion result must be used"]
pub fn i32_trunc_sat_f32_s(val: FloatBits32) -> i32 {
    let f = val.value();
    if f.is_nan() {
        0
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            i32::MAX
        } else {
            i32::MIN
        }
    } else {
        let trunc = f32_trunc_compat(f);
        // Check against the valid range for i32 represented as f32
        // i32::MIN as f32 is -2147483600.0 (approx)
        // i32::MAX as f32 is 2147483600.0 (approx)
        // A more precise range check:
        if trunc >= (i32::MIN as f32) && trunc <= (i32::MAX as f32) {
            // Check if it's precisely representable or within the range for direct cast
            if (-2_147_483_648.0_f32..2_147_483_648.0_f32).contains(&trunc) {
                // Wasm spec Table 15
                trunc as i32
            } else if trunc == -2_147_483_648.0_f32 {
                // exactly i32::MIN
                i32::MIN
            } else if f.is_sign_positive() {
                // Positive out of precise range
                i32::MAX
            } else {
                // Negative out of precise range
                i32::MIN
            }
        } else if f.is_sign_positive() {
            // Positive out of representable f32 range for i32
            i32::MAX
        } else {
            // Negative out of representable f32 range for i32
            i32::MIN
        }
    }
}

/// Converts an f32 to u32 using saturating truncation (Wasm semantics
/// `i32.trunc_sat_f32_u`). NaN -> 0. +/-Inf -> 0 if negative Inf, u32::MAX if
/// positive Inf. Out of range -> 0 or u32::MAX. Returns i32 as per Wasm return
/// type for these instructions.
#[inline]
#[must_use = "Saturating conversion result must be used"]
pub fn i32_trunc_sat_f32_u(val: FloatBits32) -> i32 {
    // Wasm returns i32
    let f = val.value();
    if f.is_nan() {
        0
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            u32::MAX as i32
        } else {
            0
        } // Negative infinity saturates to 0 for unsigned
    } else {
        let trunc = f32_trunc_compat(f);
        // Check against valid range for u32 represented as f32
        // u32::MIN (0) as f32 is 0.0
        // u32::MAX as f32 is 4294967300.0 (approx)
        if trunc > (u32::MIN as f32) && trunc < (u32::MAX as f32) {
            // Wasm spec Table 15 range
            // Further check based on spec: (0.0, 4294967296.0)
            if trunc > 0.0_f32 && trunc < 4_294_967_296.0_f32 {
                trunc as u32 as i32
            } else if trunc <= 0.0_f32 {
                // Includes -0.0
                0
            } else {
                // >= 2^32
                u32::MAX as i32
            }
        } else if trunc <= (u32::MIN as f32) {
            // Includes negative numbers and -0.0
            0
        } else {
            // Greater than or equal to u32::MAX as f32
            u32::MAX as i32
        }
    }
}

/// Converts an f32 to i64 using saturating truncation (Wasm
/// `i64.trunc_sat_f32_s`).
#[inline]
#[must_use = "Saturating conversion result must be used"]
pub fn i64_trunc_sat_f32_s(val: FloatBits32) -> i64 {
    // Wasm returns i64
    let f = val.value();
    if f.is_nan() {
        0
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            i64::MAX
        } else {
            i64::MIN
        }
    } else {
        let trunc = f32_trunc_compat(f);
        // Wasm spec Table 15 range: (-9223372036854775808.0, 9223372036854775808.0)
        // f32 cannot represent these bounds exactly.
        // If f > i64::MAX as f32 (approx 9.223372E18), saturate to i64::MAX.
        // If f < i64::MIN as f32 (approx -9.223372E18), saturate to i64::MIN.

        if (I64_MIN_AS_F32..I64_MAX_AS_F32).contains(&trunc) {
            trunc as i64
        } else if trunc == I64_MIN_AS_F32 {
            // Check exact bound for MIN
            i64::MIN
        } else if trunc >= I64_MAX_AS_F32 {
            // Handles values >= MAX bound
            i64::MAX
        } else {
            // Handles values < MIN bound
            i64::MIN
        }
    }
}

/// Converts an f32 to u64 using saturating truncation (Wasm
/// `i64.trunc_sat_f32_u`). Returns i64 as per Wasm.
#[inline]
#[must_use = "Saturating conversion result must be used"]
pub fn i64_trunc_sat_f32_u(val: FloatBits32) -> i64 {
    // Wasm returns i64
    let f = val.value();
    if f.is_nan() {
        0
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            u64::MAX as i64
        } else {
            0
        }
    } else {
        let trunc = f32_trunc_compat(f);
        // Wasm spec Table 15 range: (0.0, 18446744073709551616.0)
        const U64_MAX_AS_F32: f32 = 18_446_744_073_709_551_616.0_f32; // u64::MAX (2^64 - 1)

        if trunc > 0.0_f32 && trunc < U64_MAX_AS_F32 {
            trunc as u64 as i64
        } else if trunc <= 0.0_f32 {
            // Includes negative numbers and -0.0
            0
        } else {
            // >= U64_MAX_AS_F32
            u64::MAX as i64
        }
    }
}

/// Converts an f64 to i32 using saturating truncation (Wasm
/// `i32.trunc_sat_f64_s`). Returns i32.
#[inline]
#[must_use = "Saturating conversion result must be used"]
pub fn i32_trunc_sat_f64_s(val: FloatBits64) -> i32 {
    let f = val.value();
    if f.is_nan() {
        0
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            i32::MAX
        } else {
            i32::MIN
        }
    } else {
        let trunc = f64_trunc_compat(f);
        // Wasm spec Table 15 range: (-2147483648.0, 2147483648.0)
        if (-2_147_483_648.0_f64..2_147_483_648.0_f64).contains(&trunc) {
            trunc as i32
        } else if trunc == -2_147_483_648.0_f64 {
            // Exactly i32::MIN
            i32::MIN
        } else if trunc >= 2_147_483_648.0_f64 {
            i32::MAX
        } else {
            // trunc < -2147483648.0
            i32::MIN
        }
    }
}

/// Converts an f64 to u32 using saturating truncation (Wasm
/// `i32.trunc_sat_f64_u`). Returns i32 as per Wasm.
#[inline]
#[must_use = "Saturating conversion result must be used"]
pub fn i32_trunc_sat_f64_u(val: FloatBits64) -> i32 {
    // Wasm returns i32
    let f = val.value();
    if f.is_nan() {
        0
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            u32::MAX as i32
        } else {
            0
        }
    } else {
        let trunc = f64_trunc_compat(f);
        // Wasm spec Table 15 range: (0.0, 4294967296.0)
        if trunc > 0.0_f64 && trunc < 4_294_967_296.0_f64 {
            trunc as u32 as i32
        } else if trunc <= 0.0_f64 {
            0
        } else {
            // >= 2^32
            u32::MAX as i32
        }
    }
}

/// Converts an f64 to i64 using saturating truncation (Wasm
/// `i64.trunc_sat_f64_s`).
#[inline]
#[must_use = "Saturating conversion result must be used"]
pub fn i64_trunc_sat_f64_s(val: FloatBits64) -> i64 {
    let f = val.value();
    if f.is_nan() {
        0
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            i64::MAX
        } else {
            i64::MIN
        }
    } else {
        let trunc = f64_trunc_compat(f);
        // Wasm spec Table 15 range: (-9223372036854775808.0, 9223372036854775808.0)
        // i64::MIN is -2^63, i64::MAX is 2^63 - 1
        // For f64, these are precisely representable.
        if trunc >= (i64::MIN as f64) && trunc < (i64::MAX as f64) {
            // Max is exclusive here due to how f64 to i64 cast might handle edge.
            // If trunc is exactly i64::MAX as f64, it should cast to i64::MAX.
            // If trunc is slightly less than i64::MIN as f64, it should cast to i64::MIN.
            if trunc == (i64::MAX as f64) {
                // Check for exact MAX value
                i64::MAX
            } else {
                trunc as i64
            }
        } else if trunc == (i64::MIN as f64) {
            // Simplified boolean expression
            // Ensure exact i64::MIN
            i64::MIN
        } else if trunc >= (i64::MAX as f64) {
            i64::MAX
        } else {
            // trunc < (i64::MIN as f64)
            i64::MIN
        }
    }
}

/// Converts an f64 to u64 using saturating truncation (Wasm
/// `i64.trunc_sat_f64_u`). Returns i64 as per Wasm.
#[inline]
#[must_use = "Saturating conversion result must be used"]
pub fn i64_trunc_sat_f64_u(val: FloatBits64) -> i64 {
    // Wasm returns i64
    let f = val.value();
    if f.is_nan() {
        0
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            u64::MAX as i64
        } else {
            0
        }
    } else {
        let trunc = f64_trunc_compat(f);
        // Wasm spec Table 15 range: (0.0, 18446744073709551616.0) which is 2^64
        // u64::MAX is 2^64 - 1
        if trunc > 0.0_f64 && trunc < (u64::MAX as f64) {
            // If trunc < u64::MAX precisely
            // If trunc is precisely u64::MAX, then (trunc as u64) is u64::MAX
            // If trunc is slightly less than 2^64 but rounds to 2^64 for f64
            // then (trunc as u64) will be u64::MAX.
            // The spec range is (0, 2^64).
            if trunc >= 18_446_744_073_709_551_616.0_f64 {
                // 2^64
                u64::MAX as i64 // Saturate
            } else {
                trunc as u64 as i64
            }
        } else if trunc <= 0.0_f64 {
            0
        } else {
            // >= u64::MAX as f64, or >= 2^64
            u64::MAX as i64
        }
    }
}

// --- Trapping Float to Integer Conversions ---

/// `i32.trunc_f32_s`: Truncate f32 to signed i32.
/// Traps if the input is `NaN`, Infinity, or out of `i32` range.
///
/// # Errors
/// - `Err(TrapCode::InvalidConversionToInteger)` if `val` is `NaN` or Infinity.
/// - `Err(TrapCode::IntegerOverflow)` if the truncated value is out of `i32`
///   range.
#[inline]
pub fn i32_trunc_f32_s(val: FloatBits32) -> Result<i32> {
    let f = val.value();
    if f.is_nan() || f.is_infinite() {
        return Err(trap(TrapCode::InvalidConversionToInteger));
    }
    let f_trunc = f32_trunc_compat(f);
    // Wasm spec: "if trunc_s(z) is out of range of T, then trap"
    // Range for i32 is [-2147483648, 2147483647]
    // Check against f32 representations of these bounds.
    // f_trunc must be >= -2147483648.0 and <= 2147483647.0 (approx)
    // More precisely, use the spec's table range:
    // not in (-2147483648.0, 2147483648.0) then trap for f32->i32
    // This means f_trunc must be >= -2147483648.0 AND < 2147483648.0
    // Or more simply, if f_trunc < i32::MIN as f32 or f_trunc > i32::MAX as f32
    // (roughly)
    if !(-2_147_483_648.0_f32..2_147_483_648.0_f32).contains(&f_trunc) {
        // Special case for i32::MIN: -2147483648.0f32 as i32 is i32::MIN
        if f_trunc == -2_147_483_648.0_f32 {
            // This is valid
        } else {
            return Err(trap(TrapCode::IntegerOverflow));
        }
    }
    Ok(f_trunc as i32)
}

/// `i32.trunc_f32_u`: Truncate f32 to unsigned i32 (u32), trapping on invalid
/// input.
///
/// # Errors
/// - `Err(TrapCode::InvalidConversionToInteger)` if `val` is `NaN` or Infinity.
/// - `Err(TrapCode::IntegerOverflow)` if the truncated value is out of `u32`
///   range.
#[inline]
pub fn i32_trunc_f32_u(val: FloatBits32) -> Result<u32> {
    let f = val.value();
    if f.is_nan() || f.is_infinite() {
        return Err(trap(TrapCode::InvalidConversionToInteger));
    }
    let f_trunc = f32_trunc_compat(f);
    // Range for u32 is [0, 4294967295]
    // Wasm spec table range for f32->u32: not in (0.0, 4294967296.0) then trap
    // This means f_trunc must be >= 0.0 AND < 4294967296.0
    // (Note: -0.0 should also be handled as in range, converting to 0)
    if !(0.0_f32..4_294_967_296.0_f32).contains(&f_trunc) {
        // Check for -0.0 case explicitly, as (f_trunc < 0.0) would be true
        if f_trunc == 0.0 && f_trunc.is_sign_negative() {
            // -0.0 is fine, becomes 0u32
        } else {
            return Err(trap(TrapCode::IntegerOverflow));
        }
    }
    Ok(f_trunc as u32)
}

/// `i32.trunc_f64_s`: Truncate f64 to signed i32, trapping on invalid input.
///
/// # Errors
/// - `Err(TrapCode::InvalidConversionToInteger)` if `val` is `NaN` or Infinity.
/// - `Err(TrapCode::IntegerOverflow)` if the truncated value is out of `i32`
///   range.
#[inline]
pub fn i32_trunc_f64_s(val: FloatBits64) -> Result<i32> {
    let d = val.value();
    if d.is_nan() || d.is_infinite() {
        return Err(trap(TrapCode::InvalidConversionToInteger));
    }
    let d_trunc = f64_trunc_compat(d);
    // Wasm spec table range: not in (-2147483648.0, 2147483648.0) for f64->i32
    if !(-2_147_483_648.0_f64..2_147_483_648.0_f64).contains(&d_trunc) {
        if d_trunc == -2_147_483_648.0_f64 {
            // valid
        } else {
            return Err(trap(TrapCode::IntegerOverflow));
        }
    }
    Ok(d_trunc as i32)
}

/// `i32.trunc_f64_u`: Truncate f64 to unsigned i32 (u32), trapping on invalid
/// input.
///
/// # Errors
/// - `Err(TrapCode::InvalidConversionToInteger)` if `val` is `NaN` or Infinity.
/// - `Err(TrapCode::IntegerOverflow)` if the truncated value is out of `u32`
///   range.
#[inline]
pub fn i32_trunc_f64_u(val: FloatBits64) -> Result<u32> {
    let d = val.value();
    if d.is_nan() || d.is_infinite() {
        return Err(trap(TrapCode::InvalidConversionToInteger));
    }
    let d_trunc = f64_trunc_compat(d);
    // Wasm spec table range: not in (0.0, 4294967296.0) for f64->u32
    if !(0.0_f64..4_294_967_296.0_f64).contains(&d_trunc) {
        if d_trunc == 0.0 && d_trunc.is_sign_negative() {
            // -0.0 is fine
        } else {
            return Err(trap(TrapCode::IntegerOverflow));
        }
    }
    Ok(d_trunc as u32)
}

/// `i64.trunc_f32_s`: Truncate f32 to signed i64, trapping on invalid input.
///
/// # Errors
/// - `Err(TrapCode::InvalidConversionToInteger)` if `val` is `NaN` or Infinity.
/// - `Err(TrapCode::IntegerOverflow)` if the truncated value is out of `i64`
///   range.
#[inline]
pub fn i64_trunc_f32_s(val: FloatBits32) -> Result<i64> {
    let f = val.value();
    if f.is_nan() || f.is_infinite() {
        return Err(trap(TrapCode::InvalidConversionToInteger));
    }
    let f_trunc = f32_trunc_compat(f);
    // Wasm spec table range: not in (-9223372036854775808.0, 9223372036854775808.0)
    // for f32->i64 These bounds are not precisely representable by f32.
    // Check if f_trunc is outside the representable range of i64.
    // i64::MIN as f32 is approx -9.223372E18
    // i64::MAX as f32 is approx  9.223372E18

    if !(I64_MIN_AS_F32..I64_MAX_AS_F32).contains(&f_trunc) {
        if f_trunc == I64_MIN_AS_F32 { // Check for exact lower bound
             // This is okay, will become i64::MIN
        } else {
            return Err(trap(TrapCode::IntegerOverflow));
        }
    }
    Ok(f_trunc as i64)
}

/// `i64.trunc_f32_u`: Truncate f32 to unsigned i64 (u64), trapping on invalid
/// input.
///
/// # Errors
/// - `Err(TrapCode::InvalidConversionToInteger)` if `val` is `NaN` or Infinity.
/// - `Err(TrapCode::IntegerOverflow)` if the truncated value is out of `u64`
///   range.
#[inline]
pub fn i64_trunc_f32_u(val: FloatBits32) -> Result<u64> {
    let f = val.value();
    if f.is_nan() || f.is_infinite() {
        return Err(trap(TrapCode::InvalidConversionToInteger));
    }
    let f_trunc = f32_trunc_compat(f);
    // Wasm spec table range: not in (0.0, 18446744073709551616.0) for f32->u64
    const U64_MAX_AS_F32: f32 = 18_446_744_073_709_551_616.0_f32; // 2^64
    if !(0.0_f32..U64_MAX_AS_F32).contains(&f_trunc) {
        if f_trunc == 0.0 && f_trunc.is_sign_negative() {
            // -0.0 is fine
        } else {
            return Err(trap(TrapCode::IntegerOverflow));
        }
    }
    Ok(f_trunc as u64)
}

/// `i64.trunc_f64_s`: Truncate f64 to signed i64, trapping on invalid input.
///
/// # Errors
/// - `Err(TrapCode::InvalidConversionToInteger)` if `val` is `NaN` or Infinity.
/// - `Err(TrapCode::IntegerOverflow)` if the truncated value is out of `i64`
///   range.
#[inline]
pub fn i64_trunc_f64_s(val: FloatBits64) -> Result<i64> {
    let d = val.value();
    if d.is_nan() || d.is_infinite() {
        return Err(trap(TrapCode::InvalidConversionToInteger));
    }
    let d_trunc = f64_trunc_compat(d);
    // Wasm spec table range: not in (-9223372036854775808.0, 9223372036854775808.0)
    // for f64->i64 These are i64::MIN and (i64::MAX + 1) as f64.
    if d_trunc < (i64::MIN as f64) || d_trunc >= ((i64::MAX as f64) + 1.0) {
        // Check carefully against exact f64 repr of i64 bounds
        // Test: (i64::MIN as f64) is -9223372036854776000.0
        // Test: (i64::MAX as f64) is  9223372036854776000.0
        // The spec implies the range of representable values, not the casted bounds.
        // Let's use the numeric literals from spec's Table 15 for bounds check.
        if !(-9_223_372_036_854_775_808.0_f64..9_223_372_036_854_775_808.0_f64).contains(&d_trunc) {
            if d_trunc == -9_223_372_036_854_775_808.0_f64 { // i64::MIN
                 // okay
            } else {
                return Err(trap(TrapCode::IntegerOverflow));
            }
        }
    }
    Ok(d_trunc as i64)
}

/// `i64.trunc_f64_u`: Truncate f64 to unsigned i64 (u64), trapping on invalid
/// input.
///
/// # Errors
/// - `Err(TrapCode::InvalidConversionToInteger)` if `val` is `NaN` or Infinity.
/// - `Err(TrapCode::IntegerOverflow)` if the truncated value is out of `u64`
///   range.
#[inline]
pub fn i64_trunc_f64_u(val: FloatBits64) -> Result<u64> {
    let d = val.value();
    if d.is_nan() || d.is_infinite() {
        return Err(trap(TrapCode::InvalidConversionToInteger));
    }
    let d_trunc = f64_trunc_compat(d);
    // Wasm spec table range: not in (0.0, 18446744073709551616.0) for f64->u64
    // This is (0, 2^64).
    if !(0.0_f64..18_446_744_073_709_551_616.0_f64).contains(&d_trunc) {
        if d_trunc == 0.0 && d_trunc.is_sign_negative() {
            // -0.0 is fine
        } else {
            return Err(trap(TrapCode::IntegerOverflow));
        }
    }
    Ok(d_trunc as u64)
}

// --- Type Conversion Operations ---

// Integer to Float Conversions

/// f32.convert_i32_s: Convert signed i32 to f32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f32_convert_i32_s(val: i32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(val as f32))
}

/// f32.convert_i32_u: Convert unsigned i32 to f32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f32_convert_i32_u(val: u32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(val as f32))
}

/// f32.convert_i64_s: Convert signed i64 to f32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f32_convert_i64_s(val: i64) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(val as f32))
}

/// f32.convert_i64_u: Convert unsigned i64 to f32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f32_convert_i64_u(val: u64) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(val as f32))
}

/// f64.convert_i32_s: Convert signed i32 to f64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f64_convert_i32_s(val: i32) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(f64::from(val)))
}

/// f64.convert_i32_u: Convert unsigned i32 to f64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f64_convert_i32_u(val: u32) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(f64::from(val)))
}

/// f64.convert_i64_s: Convert signed i64 to f64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f64_convert_i64_s(val: i64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(val as f64))
}

/// f64.convert_i64_u: Convert unsigned i64 to f64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f64_convert_i64_u(val: u64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(val as f64))
}

// Float to Float Conversions

/// f32.demote_f64: Demote f64 to f32.
/// May lose precision. Follows IEEE 754 rounding rules.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f32_demote_f64(val: FloatBits64) -> Result<FloatBits32> {
    Ok(FloatBits32::from_float(val.value() as f32))
}

/// f64.promote_f32: Promote f32 to f64.
/// Exact conversion - no precision loss.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f64_promote_f32(val: FloatBits32) -> Result<FloatBits64> {
    Ok(FloatBits64::from_float(f64::from(val.value())))
}

// Reinterpret (bit casting) Operations

/// i32.reinterpret_f32: Reinterpret f32 bits as i32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_reinterpret_f32(val: FloatBits32) -> Result<i32> {
    Ok(val.to_bits() as i32)
}

/// i64.reinterpret_f64: Reinterpret f64 bits as i64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_reinterpret_f64(val: FloatBits64) -> Result<i64> {
    Ok(val.to_bits() as i64)
}

/// f32.reinterpret_i32: Reinterpret i32 bits as f32.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f32_reinterpret_i32(val: i32) -> Result<FloatBits32> {
    Ok(FloatBits32::from_bits(val as u32))
}

/// f64.reinterpret_i64: Reinterpret i64 bits as f64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn f64_reinterpret_i64(val: i64) -> Result<FloatBits64> {
    Ok(FloatBits64::from_bits(val as u64))
}

// Integer Width Conversions

/// i32.wrap_i64: Wrap i64 to i32 (truncate to lower 32 bits).
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_wrap_i64(val: i64) -> Result<i32> {
    Ok(val as i32)
}

/// i64.extend_i32_s: Sign-extend i32 to i64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_extend_i32_s(val: i32) -> Result<i64> {
    Ok(i64::from(val))
}

/// i64.extend_i32_u: Zero-extend i32 to i64.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_extend_i32_u(val: u32) -> Result<i64> {
    Ok(i64::from(val))
}

// Sign Extension Operations

/// i32.extend8_s: Sign-extend 8-bit value to 32 bits.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_extend8_s(val: i32) -> Result<i32> {
    Ok(i32::from(val as i8))
}

/// i32.extend16_s: Sign-extend 16-bit value to 32 bits.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i32_extend16_s(val: i32) -> Result<i32> {
    Ok(i32::from(val as i16))
}

/// i64.extend8_s: Sign-extend 8-bit value to 64 bits.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_extend8_s(val: i64) -> Result<i64> {
    Ok(i64::from(val as i8))
}

/// i64.extend16_s: Sign-extend 16-bit value to 64 bits.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_extend16_s(val: i64) -> Result<i64> {
    Ok(i64::from(val as i16))
}

/// i64.extend32_s: Sign-extend 32-bit value to 64 bits.
///
/// # Errors
///
/// This function does not return an error.
#[inline]
pub fn i64_extend32_s(val: i64) -> Result<i64> {
    Ok(i64::from(val as i32))
}

// TODO: Add tests for all math operations, including edge cases for
// no_std polyfills, NaN propagation, trapping conditions, and saturation.
// Consider property-based testing.
// Specific Wasm testsuite cases should be covered.
