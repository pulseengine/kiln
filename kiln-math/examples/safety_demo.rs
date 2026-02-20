// Copyright (c) 2025 R T
// SPDX-License-Identifier: MIT
// Project: WRT
// Module: wrt-math examples

//! Demonstration of ASIL-specific safety features for mathematical operations.

use wrt_math::{
    check_simd_bounds,
    RoundingMode,
    SafeArithmetic,
    SafeFloat,
    SafeRounding,
};

fn main() {
    println!("WRT Math Safety Features Demo");
    println!("=============================\n");

    // Integer arithmetic safety
    demo_integer_safety();

    // Floating-point safety
    demo_float_safety();

    // Rounding modes
    demo_rounding_modes();

    // SIMD bounds checking
    demo_simd_bounds();
}

fn demo_integer_safety() {
    println!("Integer Arithmetic Safety:");
    println!("--------------------------");

    // Safe addition
    let a = 100i32;
    let b = 200i32;
    match a.safe_add(b) {
        Ok(result) => println!("Safe add: {} + {} = {}", a, b, result),
        Err(e) => println!("Safe add failed: {:?}", e),
    }

    // Test overflow case
    let max_val = i32::MAX;
    match max_val.safe_add(1) {
        Ok(result) => {
            #[cfg(any(feature = "asil-c", feature = "asil-d"))]
            println!(
                "Overflow handled with saturation: {} + 1 = {}",
                max_val, result
            );
            #[cfg(not(any(feature = "asil-c", feature = "asil-d")))]
            println!("Overflow wrapped: {} + 1 = {}", max_val, result);
        },
        Err(e) => {
            #[cfg(any(feature = "asil-a", feature = "asil-b"))]
            println!("Overflow detected and trapped: {:?}", e);
        },
    }

    // Safe division
    match 10i32.safe_div(3) {
        Ok(result) => println!("Safe division: 10 / 3 = {}", result),
        Err(e) => println!("Division failed: {:?}", e),
    }

    // Division by zero
    match 10i32.safe_div(0) {
        Ok(result) => println!("Division by zero: 10 / 0 = {}", result),
        Err(e) => println!("Division by zero trapped: {:?}", e),
    }

    println!();
}

fn demo_float_safety() {
    println!("Floating-Point Safety:");
    println!("----------------------");

    let a = core::f32::consts::PI;
    let b = 2.71f32;

    match a.safe_float_op(b, |x, y| x + y) {
        Ok(result) => println!("Safe float operation: {} + {} = {}", a, b, result),
        Err(e) => println!("Float operation failed: {:?}", e),
    }

    // Test with NaN (if NaN checking is enabled)
    let nan_val = f32::NAN;
    match nan_val.safe_float_op(1.0, |x, y| x + y) {
        Ok(result) => println!("NaN operation result: {}", result),
        Err(e) => {
            #[cfg(feature = "nan-propagation-checking")]
            println!("NaN detected and trapped: {:?}", e);
            #[cfg(not(feature = "nan-propagation-checking"))]
            println!("NaN checking disabled: {:?}", e);
        },
    }

    println!();
}

fn demo_rounding_modes() {
    println!("Rounding Modes:");
    println!("---------------");

    let val = 2.7f32;

    let modes = [
        (RoundingMode::NearestEven, "Nearest (ties to even)"),
        (RoundingMode::TowardZero, "Toward zero"),
        (RoundingMode::TowardPositive, "Toward +∞"),
        (RoundingMode::TowardNegative, "Toward -∞"),
    ];

    for (mode, description) in modes.iter() {
        match val.safe_round(*mode) {
            Ok(result) => println!("{}: {} -> {}", description, val, result),
            Err(e) => println!("{}: Error - {:?}", description, e),
        }
    }

    println!();
}

fn demo_simd_bounds() {
    println!("SIMD Bounds Checking:");
    println!("---------------------");

    let memory_size = 1024;

    // Valid access
    match check_simd_bounds(0, 64, memory_size) {
        Ok(()) => println!(
            "Valid SIMD access: offset=0, len=64, memory_size={}",
            memory_size
        ),
        Err(e) => println!("SIMD bounds check failed: {:?}", e),
    }

    // Out of bounds access
    match check_simd_bounds(1000, 64, memory_size) {
        Ok(()) => println!(
            "SIMD access allowed: offset=1000, len=64, memory_size={}",
            memory_size
        ),
        Err(e) => {
            #[cfg(feature = "runtime-bounds-checking")]
            println!("Out of bounds SIMD access prevented: {:?}", e);
            #[cfg(not(feature = "runtime-bounds-checking"))]
            println!("SIMD bounds checking disabled");
        },
    }

    println!();
}
