//! Demonstration of WebAssembly arithmetic operations
//!
//! This example shows how to use:
//! - Integer arithmetic operations (add, sub, mul, div, bitwise)
//! - Floating-point arithmetic operations (add, sub, mul, div, min, max, abs,
//!   etc.)
//! - Math operations (sqrt, ceil, floor, trunc, nearest)
//! - Bit counting operations (clz, ctz, popcnt)

// Allow 8-bit grouping for binary literals in bit manipulation demos
#![allow(clippy::unreadable_literal)]

use std::vec::Vec;

use wrt_error::Result;
use wrt_foundation::{
    FloatBits32,
    FloatBits64,
    Value,
};
use wrt_instructions::{
    ArithmeticContext,
    ArithmeticOp,
    PureInstruction,
};

// Mock execution context for demonstration
#[cfg(feature = "std")]
struct DemoContext {
    stack: Vec<Value>,
}

#[cfg(feature = "std")]
impl DemoContext {
    fn new() -> Self {
        Self { stack: Vec::new() }
    }

    fn peek(&self) -> Option<&Value> {
        self.stack.last()
    }
}

#[cfg(feature = "std")]
impl ArithmeticContext for DemoContext {
    fn pop_arithmetic_value(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| wrt_error::Error::runtime_error("Stack underflow"))
    }

    fn push_arithmetic_value(&mut self, value: Value) -> Result<()> {
        self.stack.push(value);
        Ok(())
    }
}

#[cfg(feature = "std")]
fn main() -> Result<()> {
    println!("=== WebAssembly Arithmetic Operations Demo ===\n");

    let mut context = DemoContext::new();

    // 1. Integer arithmetic (i32)
    println!("1. Integer Arithmetic (i32):");
    context.push_arithmetic_value(Value::I32(15))?;
    context.push_arithmetic_value(Value::I32(7))?;
    println!("   Input: 15, 7");

    // Add
    ArithmeticOp::I32Add.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   15 + 7 = {}", result);
    }
    context.stack.clear();

    // Subtract
    context.push_arithmetic_value(Value::I32(15))?;
    context.push_arithmetic_value(Value::I32(7))?;
    ArithmeticOp::I32Sub.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   15 - 7 = {}", result);
    }
    context.stack.clear();

    // Multiply
    context.push_arithmetic_value(Value::I32(15))?;
    context.push_arithmetic_value(Value::I32(7))?;
    ArithmeticOp::I32Mul.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   15 * 7 = {}", result);
    }
    context.stack.clear();

    // Divide (signed)
    context.push_arithmetic_value(Value::I32(15))?;
    context.push_arithmetic_value(Value::I32(7))?;
    ArithmeticOp::I32DivS.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   15 / 7 = {} (signed)", result);
    }
    context.stack.clear();

    // 2. Bitwise operations
    println!("\n2. Bitwise Operations (i32):");
    context.push_arithmetic_value(Value::I32(0b1010))?; // 10
    context.push_arithmetic_value(Value::I32(0b1100))?; // 12
    println!("   Input: 0b1010 (10), 0b1100 (12)");

    ArithmeticOp::I32And.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   10 & 12 = {} (0b{:04b})", result, result);
    }
    context.stack.clear();

    context.push_arithmetic_value(Value::I32(0b1010))?;
    context.push_arithmetic_value(Value::I32(0b1100))?;
    ArithmeticOp::I32Or.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   10 | 12 = {} (0b{:04b})", result, result);
    }
    context.stack.clear();

    context.push_arithmetic_value(Value::I32(0b1010))?;
    context.push_arithmetic_value(Value::I32(0b1100))?;
    ArithmeticOp::I32Xor.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   10 ^ 12 = {} (0b{:04b})", result, result);
    }
    context.stack.clear();

    // 3. Bit counting operations
    println!("\n3. Bit Counting Operations:");

    // Count leading zeros
    context.push_arithmetic_value(Value::I32(0b00000000_00000000_00000000_00001000))?; // 8
    println!("   Input: 8 (0b00000000000000000000000000001000)");
    ArithmeticOp::I32Clz.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Count leading zeros: {}", result);
    }
    context.stack.clear();

    // Count trailing zeros
    context.push_arithmetic_value(Value::I32(0b00001000_00000000_00000000_00000000))?;
    println!("   Input: 134_217_728 (bit 27 set)");
    ArithmeticOp::I32Ctz.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Count trailing zeros: {}", result);
    }
    context.stack.clear();

    // Population count (count set bits)
    context.push_arithmetic_value(Value::I32(0b01010101_01010101_01010101_01010101))?;
    println!("   Input: alternating bits pattern");
    ArithmeticOp::I32Popcnt.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Population count (set bits): {}", result);
    }
    context.stack.clear();

    // 4. Float arithmetic (f32)
    println!("\n4. Float Arithmetic (f32):");
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(3.14)))?;
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.71)))?;
    println!("   Input: 3.14, 2.71");

    ArithmeticOp::F32Add.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   3.14 + 2.71 = {}", result.value);
    }
    context.stack.clear();

    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(10.0)))?;
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(3.0)))?;
    ArithmeticOp::F32Div.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   10.0 / 3.0 = {}", result.value);
    }
    context.stack.clear();

    // 5. Float math operations
    println!("\n5. Float Math Operations:");

    // Square root
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(16.0)))?;
    println!("   Input: 16.0");
    ArithmeticOp::F32Sqrt.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   sqrt(16.0) = {}", result.value);
    }
    context.stack.clear();

    // Absolute value
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(-42.5)))?;
    println!("   Input: -42.5");
    ArithmeticOp::F32Abs.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   abs(-42.5) = {}", result.value);
    }
    context.stack.clear();

    // Ceiling
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.3)))?;
    println!("   Input: 2.3");
    ArithmeticOp::F32Ceil.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   ceil(2.3) = {}", result.value);
    }
    context.stack.clear();

    // Floor
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.8)))?;
    println!("   Input: 2.8");
    ArithmeticOp::F32Floor.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   floor(2.8) = {}", result.value);
    }
    context.stack.clear();

    // Truncate
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(-2.8)))?;
    println!("   Input: -2.8");
    ArithmeticOp::F32Trunc.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   trunc(-2.8) = {} (towards zero)", result.value);
    }
    context.stack.clear();

    // Nearest (round to even)
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(2.5)))?;
    println!("   Input: 2.5");
    ArithmeticOp::F32Nearest.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   nearest(2.5) = {} (round to even)", result.value);
    }
    context.stack.clear();

    // 6. Min/Max operations
    println!("\n6. Min/Max Operations:");
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(5.7)))?;
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(3.2)))?;
    println!("   Input: 5.7, 3.2");

    ArithmeticOp::F32Min.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   min(5.7, 3.2) = {}", result.value);
    }
    context.stack.clear();

    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(5.7)))?;
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(3.2)))?;
    ArithmeticOp::F32Max.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   max(5.7, 3.2) = {}", result.value);
    }
    context.stack.clear();

    // 7. Sign operations
    println!("\n7. Sign Operations:");

    // Negate
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(42.0)))?;
    println!("   Input: 42.0");
    ArithmeticOp::F32Neg.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   neg(42.0) = {}", result.value);
    }
    context.stack.clear();

    // Copy sign
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(42.0)))?;
    context.push_arithmetic_value(Value::F32(FloatBits32::from_float(-1.0)))?;
    println!("   Input: 42.0, -1.0");
    ArithmeticOp::F32Copysign.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!(
            "   copysign(42.0, -1.0) = {} (42.0 with sign of -1.0)",
            result.value
        );
    }
    context.stack.clear();

    // 8. i64 operations example
    println!("\n8. 64-bit Integer Operations:");
    context.push_arithmetic_value(Value::I64(0x1234567890ABCDEF))?;
    context.push_arithmetic_value(Value::I64(0x1111111111111111))?;
    println!("   Input: 0x1234567890ABCDEF, 0x1111111111111111");

    ArithmeticOp::I64Add.execute(&mut context)?;
    if let Some(Value::I64(result)) = context.peek() {
        println!("   Add result: 0x{:016X}", result);
    }
    context.stack.clear();

    // 9. f64 operations example
    println!("\n9. 64-bit Float Operations:");
    context.push_arithmetic_value(Value::F64(FloatBits64::from_float(3.141592653589793)))?;
    context.push_arithmetic_value(Value::F64(FloatBits64::from_float(2.718281828459045)))?;
    println!("   Input: π (3.141592653589793), e (2.718281828459045)");

    ArithmeticOp::F64Add.execute(&mut context)?;
    if let Some(Value::F64(result)) = context.peek() {
        println!("   π + e = {}", result.value);
    }
    context.stack.clear();

    context.push_arithmetic_value(Value::F64(FloatBits64::from_float(2.0)))?;
    println!("   Input: 2.0");
    ArithmeticOp::F64Sqrt.execute(&mut context)?;
    if let Some(Value::F64(result)) = context.peek() {
        println!("   sqrt(2.0) = {}", result.value);
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() {
    // Binary std/no_std choice
    eprintln!("This example requires std or alloc features");
}
