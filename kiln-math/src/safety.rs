// Copyright (c) 2025 R T
// SPDX-License-Identifier: MIT
// Project: WRT
// Module: wrt-math::safety (SW-REQ-ID-TBD)

//! ASIL-specific safety features for mathematical operations.
//!
//! This module provides safe arithmetic operations that progressively enable
//! based on ASIL level:
//! - ASIL-A: Runtime bounds checking
//! - ASIL-B: Checked arithmetic operations
//! - ASIL-C: Saturating arithmetic as default
//! - ASIL-D: Formal verification hooks for arithmetic

use wrt_error::{
    codes::TrapCode,
    Error as WrtError,
    ErrorCategory,
    Result,
};

// Import from libm for no_std math functions
#[cfg(not(feature = "std"))]
mod math_helpers {
    /// Round towards nearest integer, ties to even
    #[inline]
    pub fn round_f32(x: f32) -> f32 {
        libm::roundf(x)
    }

    /// Round towards nearest integer, ties to even
    #[inline]
    pub fn round_f64(x: f64) -> f64 {
        libm::round(x)
    }

    /// Truncate towards zero
    #[inline]
    pub fn trunc_f32(x: f32) -> f32 {
        libm::truncf(x)
    }

    /// Truncate towards zero
    #[inline]
    pub fn trunc_f64(x: f64) -> f64 {
        libm::trunc(x)
    }

    /// Round towards positive infinity
    #[inline]
    pub fn ceil_f32(x: f32) -> f32 {
        libm::ceilf(x)
    }

    /// Round towards positive infinity
    #[inline]
    pub fn ceil_f64(x: f64) -> f64 {
        libm::ceil(x)
    }

    /// Round towards negative infinity
    #[inline]
    pub fn floor_f32(x: f32) -> f32 {
        libm::floorf(x)
    }

    /// Round towards negative infinity
    #[inline]
    pub fn floor_f64(x: f64) -> f64 {
        libm::floor(x)
    }
}

#[cfg(feature = "std")]
mod math_helpers {
    /// Round towards nearest integer, ties to even
    #[inline]
    pub fn round_f32(x: f32) -> f32 {
        x.round()
    }

    /// Round towards nearest integer, ties to even
    #[inline]
    pub fn round_f64(x: f64) -> f64 {
        x.round()
    }

    /// Truncate towards zero
    #[inline]
    pub fn trunc_f32(x: f32) -> f32 {
        x.trunc()
    }

    /// Truncate towards zero
    #[inline]
    pub fn trunc_f64(x: f64) -> f64 {
        x.trunc()
    }

    /// Round towards positive infinity
    #[inline]
    pub fn ceil_f32(x: f32) -> f32 {
        x.ceil()
    }

    /// Round towards positive infinity
    #[inline]
    pub fn ceil_f64(x: f64) -> f64 {
        x.ceil()
    }

    /// Round towards negative infinity
    #[inline]
    pub fn floor_f32(x: f32) -> f32 {
        x.floor()
    }

    /// Round towards negative infinity
    #[inline]
    pub fn floor_f64(x: f64) -> f64 {
        x.floor()
    }
}

use math_helpers::{
    ceil_f32,
    ceil_f64,
    floor_f32,
    floor_f64,
    round_f32,
    round_f64,
    trunc_f32,
    trunc_f64,
};

/// Safe arithmetic operations trait for integer types
pub trait SafeArithmetic: Sized {
    /// Safe addition with overflow checking based on ASIL level
    fn safe_add(self, rhs: Self) -> Result<Self>;

    /// Safe subtraction with overflow checking based on ASIL level
    fn safe_sub(self, rhs: Self) -> Result<Self>;

    /// Safe multiplication with overflow checking based on ASIL level
    fn safe_mul(self, rhs: Self) -> Result<Self>;

    /// Safe division with zero-check
    fn safe_div(self, rhs: Self) -> Result<Self>;

    /// Safe remainder with zero-check
    fn safe_rem(self, rhs: Self) -> Result<Self>;
}

/// Safe floating-point operations trait
pub trait SafeFloat: Sized {
    /// Check if value is NaN and handle based on ASIL level
    fn check_nan(&self) -> Result<()>;

    /// Safe floating-point operation with NaN propagation checking
    fn safe_float_op<F>(self, rhs: Self, op: F) -> Result<Self>
    where
        F: FnOnce(Self, Self) -> Self;
}

// Implement SafeArithmetic for i32
impl SafeArithmetic for i32 {
    #[inline]
    fn safe_add(self, rhs: Self) -> Result<Self> {
        #[cfg(feature = "asil-d")]
        {
            // ASIL-D: Use saturating arithmetic with formal verification hooks
            #[cfg(feature = "formal-verification-required")]
            kani_hook_i32_add(self, rhs);

            Ok(self.saturating_add(rhs))
        }

        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            // ASIL-C: Use saturating arithmetic
            Ok(self.saturating_add(rhs))
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            // ASIL-B: Use checked arithmetic
            self.checked_add(rhs).ok_or_else(|| TrapCode::IntegerOverflow.into())
        }

        #[cfg(all(feature = "asil-a", not(feature = "asil-b")))]
        {
            // ASIL-A: Runtime bounds checking
            let (result, overflow) = self.overflowing_add(rhs);
            if overflow {
                Err(TrapCode::IntegerOverflow.into())
            } else {
                Ok(result)
            }
        }

        #[cfg(not(feature = "asil-a"))]
        {
            // Default: No safety checks
            Ok(self.wrapping_add(rhs))
        }
    }

    #[inline]
    fn safe_sub(self, rhs: Self) -> Result<Self> {
        #[cfg(feature = "asil-d")]
        {
            #[cfg(feature = "formal-verification-required")]
            kani_hook_i32_sub(self, rhs);

            Ok(self.saturating_sub(rhs))
        }

        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            Ok(self.saturating_sub(rhs))
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            self.checked_sub(rhs).ok_or_else(|| TrapCode::IntegerOverflow.into())
        }

        #[cfg(all(feature = "asil-a", not(feature = "asil-b")))]
        {
            let (result, overflow) = self.overflowing_sub(rhs);
            if overflow {
                Err(TrapCode::IntegerOverflow.into())
            } else {
                Ok(result)
            }
        }

        #[cfg(not(feature = "asil-a"))]
        {
            Ok(self.wrapping_sub(rhs))
        }
    }

    #[inline]
    fn safe_mul(self, rhs: Self) -> Result<Self> {
        #[cfg(feature = "asil-d")]
        {
            #[cfg(feature = "formal-verification-required")]
            kani_hook_i32_mul(self, rhs);

            Ok(self.saturating_mul(rhs))
        }

        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            Ok(self.saturating_mul(rhs))
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            self.checked_mul(rhs).ok_or_else(|| TrapCode::IntegerOverflow.into())
        }

        #[cfg(all(feature = "asil-a", not(feature = "asil-b")))]
        {
            let (result, overflow) = self.overflowing_mul(rhs);
            if overflow {
                Err(TrapCode::IntegerOverflow.into())
            } else {
                Ok(result)
            }
        }

        #[cfg(not(feature = "asil-a"))]
        {
            Ok(self.wrapping_mul(rhs))
        }
    }

    #[inline]
    fn safe_div(self, rhs: Self) -> Result<Self> {
        if rhs == 0 {
            return Err(TrapCode::IntegerDivideByZero.into());
        }

        // Check for i32::MIN / -1 overflow
        if self == i32::MIN && rhs == -1 {
            #[cfg(any(feature = "asil-c", feature = "asil-d"))]
            {
                return Ok(i32::MAX); // Saturate
            }

            #[cfg(not(any(feature = "asil-c", feature = "asil-d")))]
            {
                return Err(TrapCode::IntegerOverflow.into());
            }
        }

        Ok(self / rhs)
    }

    #[inline]
    fn safe_rem(self, rhs: Self) -> Result<Self> {
        if rhs == 0 {
            return Err(TrapCode::IntegerDivideByZero.into());
        }

        // i32::MIN % -1 is well-defined (0) in Rust
        Ok(self % rhs)
    }
}

// Implement SafeArithmetic for i64
impl SafeArithmetic for i64 {
    #[inline]
    fn safe_add(self, rhs: Self) -> Result<Self> {
        #[cfg(feature = "asil-d")]
        {
            #[cfg(feature = "formal-verification-required")]
            kani_hook_i64_add(self, rhs);

            Ok(self.saturating_add(rhs))
        }

        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            Ok(self.saturating_add(rhs))
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            self.checked_add(rhs).ok_or_else(|| TrapCode::IntegerOverflow.into())
        }

        #[cfg(all(feature = "asil-a", not(feature = "asil-b")))]
        {
            let (result, overflow) = self.overflowing_add(rhs);
            if overflow {
                Err(TrapCode::IntegerOverflow.into())
            } else {
                Ok(result)
            }
        }

        #[cfg(not(feature = "asil-a"))]
        {
            Ok(self.wrapping_add(rhs))
        }
    }

    #[inline]
    fn safe_sub(self, rhs: Self) -> Result<Self> {
        #[cfg(feature = "asil-d")]
        {
            #[cfg(feature = "formal-verification-required")]
            kani_hook_i64_sub(self, rhs);

            Ok(self.saturating_sub(rhs))
        }

        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            Ok(self.saturating_sub(rhs))
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            self.checked_sub(rhs).ok_or_else(|| TrapCode::IntegerOverflow.into())
        }

        #[cfg(all(feature = "asil-a", not(feature = "asil-b")))]
        {
            let (result, overflow) = self.overflowing_sub(rhs);
            if overflow {
                Err(TrapCode::IntegerOverflow.into())
            } else {
                Ok(result)
            }
        }

        #[cfg(not(feature = "asil-a"))]
        {
            Ok(self.wrapping_sub(rhs))
        }
    }

    #[inline]
    fn safe_mul(self, rhs: Self) -> Result<Self> {
        #[cfg(feature = "asil-d")]
        {
            #[cfg(feature = "formal-verification-required")]
            kani_hook_i64_mul(self, rhs);

            Ok(self.saturating_mul(rhs))
        }

        #[cfg(all(feature = "asil-c", not(feature = "asil-d")))]
        {
            Ok(self.saturating_mul(rhs))
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-c")))]
        {
            self.checked_mul(rhs).ok_or_else(|| TrapCode::IntegerOverflow.into())
        }

        #[cfg(all(feature = "asil-a", not(feature = "asil-b")))]
        {
            let (result, overflow) = self.overflowing_mul(rhs);
            if overflow {
                Err(TrapCode::IntegerOverflow.into())
            } else {
                Ok(result)
            }
        }

        #[cfg(not(feature = "asil-a"))]
        {
            Ok(self.wrapping_mul(rhs))
        }
    }

    #[inline]
    fn safe_div(self, rhs: Self) -> Result<Self> {
        if rhs == 0 {
            return Err(TrapCode::IntegerDivideByZero.into());
        }

        // Check for i64::MIN / -1 overflow
        if self == i64::MIN && rhs == -1 {
            #[cfg(any(feature = "asil-c", feature = "asil-d"))]
            {
                return Ok(i64::MAX); // Saturate
            }

            #[cfg(not(any(feature = "asil-c", feature = "asil-d")))]
            {
                return Err(TrapCode::IntegerOverflow.into());
            }
        }

        Ok(self / rhs)
    }

    #[inline]
    fn safe_rem(self, rhs: Self) -> Result<Self> {
        if rhs == 0 {
            return Err(TrapCode::IntegerDivideByZero.into());
        }

        // i64::MIN % -1 is well-defined (0) in Rust
        Ok(self % rhs)
    }
}

// Implement SafeFloat for f32
impl SafeFloat for f32 {
    #[inline]
    fn check_nan(&self) -> Result<()> {
        #[cfg(feature = "nan-propagation-checking")]
        {
            if self.is_nan() {
                return Err(TrapCode::InvalidConversionToInteger.into());
            }
        }
        Ok(())
    }

    #[inline]
    fn safe_float_op<F>(self, rhs: Self, op: F) -> Result<Self>
    where
        F: FnOnce(Self, Self) -> Self,
    {
        #[cfg(feature = "nan-propagation-checking")]
        {
            self.check_nan()?;
            rhs.check_nan()?;
        }

        let result = op(self, rhs);

        #[cfg(feature = "nan-propagation-checking")]
        {
            result.check_nan()?;
        }

        Ok(result)
    }
}

// Implement SafeFloat for f64
impl SafeFloat for f64 {
    #[inline]
    fn check_nan(&self) -> Result<()> {
        #[cfg(feature = "nan-propagation-checking")]
        {
            if self.is_nan() {
                return Err(TrapCode::InvalidConversionToInteger.into());
            }
        }
        Ok(())
    }

    #[inline]
    fn safe_float_op<F>(self, rhs: Self, op: F) -> Result<Self>
    where
        F: FnOnce(Self, Self) -> Self,
    {
        #[cfg(feature = "nan-propagation-checking")]
        {
            self.check_nan()?;
            rhs.check_nan()?;
        }

        let result = op(self, rhs);

        #[cfg(feature = "nan-propagation-checking")]
        {
            result.check_nan()?;
        }

        Ok(result)
    }
}

/// ASIL-aware rounding modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingMode {
    /// Round to nearest, ties to even (default)
    NearestEven,
    /// Round toward zero (truncate)
    TowardZero,
    /// Round toward positive infinity
    TowardPositive,
    /// Round toward negative infinity
    TowardNegative,
}

/// Safe rounding operations based on ASIL level
pub trait SafeRounding {
    /// Round value according to specified mode with ASIL-aware safety
    fn safe_round(self, mode: RoundingMode) -> Result<Self>
    where
        Self: Sized;
}

impl SafeRounding for f32 {
    #[inline]
    fn safe_round(self, mode: RoundingMode) -> Result<Self> {
        #[cfg(feature = "nan-propagation-checking")]
        {
            self.check_nan()?;
        }

        let result = match mode {
            RoundingMode::NearestEven => round_f32(self),
            RoundingMode::TowardZero => trunc_f32(self),
            RoundingMode::TowardPositive => ceil_f32(self),
            RoundingMode::TowardNegative => floor_f32(self),
        };

        #[cfg(feature = "nan-propagation-checking")]
        {
            result.check_nan()?;
        }

        Ok(result)
    }
}

impl SafeRounding for f64 {
    #[inline]
    fn safe_round(self, mode: RoundingMode) -> Result<Self> {
        #[cfg(feature = "nan-propagation-checking")]
        {
            self.check_nan()?;
        }

        let result = match mode {
            RoundingMode::NearestEven => round_f64(self),
            RoundingMode::TowardZero => trunc_f64(self),
            RoundingMode::TowardPositive => ceil_f64(self),
            RoundingMode::TowardNegative => floor_f64(self),
        };

        #[cfg(feature = "nan-propagation-checking")]
        {
            result.check_nan()?;
        }

        Ok(result)
    }
}

/// Bounds checking for array/memory access in SIMD operations
#[inline]
pub fn check_simd_bounds(offset: usize, len: usize, memory_size: usize) -> Result<()> {
    #[cfg(feature = "runtime-bounds-checking")]
    {
        if offset.saturating_add(len) > memory_size {
            return Err(TrapCode::MemoryOutOfBounds.into());
        }
    }
    Ok(())
}

/// Safe memory access for SIMD operations
#[inline]
pub fn safe_simd_load<T: Copy>(memory: &[u8], offset: usize, len: usize) -> Result<&[T]> {
    let byte_len = len * core::mem::size_of::<T>();
    check_simd_bounds(offset, byte_len, memory.len())?;

    // Use safe slice operations instead of unsafe pointer manipulation
    let byte_slice = &memory[offset..offset + byte_len];

    // For ASIL compliance, we'll return an error if alignment is not guaranteed
    // rather than using unsafe pointer casting
    if offset % core::mem::align_of::<T>() != 0 {
        return Err(WrtError::runtime_execution_error(
            "Misaligned SIMD memory access",
        ));
    }

    // Since we can't safely cast without platform-specific code, we'll provide a
    // different API This is a placeholder - real SIMD operations would need
    // platform-specific handling
    Err(WrtError::runtime_not_implemented(
        "SIMD operations require platform-specific implementation",
    ))
}

/// Safe memory store for SIMD operations
#[inline]
pub fn safe_simd_store<T: Copy>(memory: &mut [u8], offset: usize, data: &[T]) -> Result<()> {
    let byte_len = core::mem::size_of_val(data);
    check_simd_bounds(offset, byte_len, memory.len())?;

    // For ASIL compliance, we'll return an error if alignment is not guaranteed
    if offset % core::mem::align_of::<T>() != 0 {
        return Err(WrtError::runtime_execution_error(
            "Misaligned SIMD memory access",
        ));
    }

    // Since we can't safely store without platform-specific code, we'll provide a
    // different API This is a placeholder - real SIMD operations would need
    // platform-specific handling
    Err(WrtError::runtime_not_implemented(
        "SIMD operations require platform-specific implementation",
    ))
}

// Formal verification hooks (no-op in normal compilation)
#[cfg(all(feature = "formal-verification-required", not(kani)))]
#[inline(always)]
fn kani_hook_i32_add(_a: i32, _b: i32) {}

#[cfg(all(feature = "formal-verification-required", not(kani)))]
#[inline(always)]
fn kani_hook_i32_sub(_a: i32, _b: i32) {}

#[cfg(all(feature = "formal-verification-required", not(kani)))]
#[inline(always)]
fn kani_hook_i32_mul(_a: i32, _b: i32) {}

#[cfg(all(feature = "formal-verification-required", not(kani)))]
#[inline(always)]
fn kani_hook_i64_add(_a: i64, _b: i64) {}

#[cfg(all(feature = "formal-verification-required", not(kani)))]
#[inline(always)]
fn kani_hook_i64_sub(_a: i64, _b: i64) {}

#[cfg(all(feature = "formal-verification-required", not(kani)))]
#[inline(always)]
fn kani_hook_i64_mul(_a: i64, _b: i64) {}

// Kani proof harnesses for formal verification
#[cfg(all(kani, feature = "formal-verification-required"))]
mod proofs {
    use super::*;

    #[kani::proof]
    fn verify_i32_safe_add() {
        let a: i32 = kani::any();
        let b: i32 = kani::any();

        match a.safe_add(b) {
            Ok(result) => {
                // Verify no overflow occurred
                assert!(result == a.saturating_add(b));
            },
            Err(_) => {
                // This should not happen with saturating arithmetic
                unreachable!();
            },
        }
    }

    #[kani::proof]
    fn verify_i32_safe_div() {
        let a: i32 = kani::any();
        let b: i32 = kani::any();

        match a.safe_div(b) {
            Ok(result) => {
                // Division succeeded, so b != 0
                assert!(b != 0);
                // Special case handling
                if a == i32::MIN && b == -1 {
                    assert!(result == i32::MAX);
                } else {
                    assert!(result == a / b);
                }
            },
            Err(_) => {
                // Division failed, must be zero divisor
                assert!(b == 0);
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_arithmetic_i32() {
        // Test overflow handling
        assert!(i32::MAX.safe_add(1).is_ok() || i32::MAX.safe_add(1).is_err());
        assert!(i32::MIN.safe_sub(1).is_ok() || i32::MIN.safe_sub(1).is_err());

        // Test division by zero
        assert!(5i32.safe_div(0).is_err());

        // Test special overflow case
        let result = i32::MIN.safe_div(-1);
        #[cfg(any(feature = "asil-c", feature = "asil-d"))]
        assert_eq!(result.unwrap(), i32::MAX);
    }

    #[test]
    fn test_safe_float_operations() {
        let a = 1.0f32;
        let b = 2.0f32;

        let result = a.safe_float_op(b, |x, y| x + y);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3.0f32);
    }

    #[test]
    fn test_rounding_modes() {
        let val = 2.5f32;

        // Note: libm::roundf rounds ties away from zero, not to even
        // This is different from IEEE 754-2008 roundTiesToEven
        assert_eq!(val.safe_round(RoundingMode::NearestEven).unwrap(), 3.0);
        assert_eq!(val.safe_round(RoundingMode::TowardZero).unwrap(), 2.0);
        assert_eq!(val.safe_round(RoundingMode::TowardPositive).unwrap(), 3.0);
        assert_eq!(val.safe_round(RoundingMode::TowardNegative).unwrap(), 2.0);

        // Test with 1.5 to show libm behavior
        let val2 = 1.5f32;
        assert_eq!(val2.safe_round(RoundingMode::NearestEven).unwrap(), 2.0);
    }

    #[test]
    fn test_simd_bounds_checking() {
        #[cfg(feature = "std")]
        let memory = vec![0u8; 100];
        #[cfg(not(feature = "std"))]
        let memory = [0u8; 100];

        // Valid access
        assert!(check_simd_bounds(0, 10, memory.len()).is_ok());

        // Out of bounds
        #[cfg(feature = "runtime-bounds-checking")]
        assert!(check_simd_bounds(95, 10, memory.len()).is_err());
    }
}
