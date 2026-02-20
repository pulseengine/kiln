//! Demonstration of WebAssembly comparison operations
//!
//! This example shows how to use:
//! - Integer comparison operations (equality, relational signed/unsigned)
//! - Floating-point comparison operations (equality, relational with NaN
//!   handling)
//! - Test operations (eqz for testing zero values)
//! - Edge cases and WebAssembly-specific semantics

#[cfg(feature = "std")]
use std::vec::Vec;

use wrt_error::Result;
use wrt_foundation::{
    FloatBits32,
    FloatBits64,
    Value,
};
use wrt_instructions::{
    ComparisonContext,
    ComparisonOp,
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
impl ComparisonContext for DemoContext {
    fn pop_comparison_value(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| wrt_error::Error::runtime_error("Stack underflow"))
    }

    fn push_comparison_value(&mut self, value: Value) -> Result<()> {
        self.stack.push(value);
        Ok(())
    }
}

#[cfg(feature = "std")]
fn main() -> Result<()> {
    println!("=== WebAssembly Comparison Operations Demo ===\n");

    let mut context = DemoContext::new();

    // 1. Integer equality comparisons (i32)
    println!("1. Integer Equality Comparisons (i32):");

    // i32.eq (equal)
    context.push_comparison_value(Value::I32(42))?;
    context.push_comparison_value(Value::I32(42))?;
    println!("   Input: 42, 42");
    ComparisonOp::I32Eq.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   42 == 42: {} (true)", result);
    }
    context.stack.clear();

    // i32.ne (not equal)
    context.push_comparison_value(Value::I32(42))?;
    context.push_comparison_value(Value::I32(13))?;
    println!("   Input: 42, 13");
    ComparisonOp::I32Ne.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   42 != 13: {} (true)", result);
    }
    context.stack.clear();

    // 2. Integer relational comparisons (signed)
    println!("\n2. Integer Relational Comparisons (Signed):");

    // i32.lt_s (less than, signed)
    context.push_comparison_value(Value::I32(-10))?;
    context.push_comparison_value(Value::I32(5))?;
    println!("   Input: -10, 5");
    ComparisonOp::I32LtS.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   -10 < 5 (signed): {} (true)", result);
    }
    context.stack.clear();

    // i32.gt_s (greater than, signed)
    context.push_comparison_value(Value::I32(100))?;
    context.push_comparison_value(Value::I32(-5))?;
    println!("   Input: 100, -5");
    ComparisonOp::I32GtS.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   100 > -5 (signed): {} (true)", result);
    }
    context.stack.clear();

    // i32.le_s (less than or equal, signed)
    context.push_comparison_value(Value::I32(7))?;
    context.push_comparison_value(Value::I32(7))?;
    println!("   Input: 7, 7");
    ComparisonOp::I32LeS.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   7 <= 7 (signed): {} (true)", result);
    }
    context.stack.clear();

    // i32.ge_s (greater than or equal, signed)
    context.push_comparison_value(Value::I32(10))?;
    context.push_comparison_value(Value::I32(7))?;
    println!("   Input: 10, 7");
    ComparisonOp::I32GeS.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   10 >= 7 (signed): {} (true)", result);
    }
    context.stack.clear();

    // 3. Integer relational comparisons (unsigned)
    println!("\n3. Integer Relational Comparisons (Unsigned):");

    // i32.lt_u (less than, unsigned) - showing signed vs unsigned difference
    context.push_comparison_value(Value::I32(-1))?; // 0xFFFFFFFF as unsigned
    context.push_comparison_value(Value::I32(10))?;
    println!("   Input: -1 (0xFFFFFFFF), 10");
    ComparisonOp::I32LtU.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!(
            "   -1 < 10 (unsigned): {} (false, -1 as unsigned is very large)",
            result
        );
    }
    context.stack.clear();

    // i32.gt_u (greater than, unsigned)
    context.push_comparison_value(Value::I32(-1))?;
    context.push_comparison_value(Value::I32(10))?;
    println!("   Input: -1 (0xFFFFFFFF), 10");
    ComparisonOp::I32GtU.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!(
            "   -1 > 10 (unsigned): {} (true, -1 as unsigned is very large)",
            result
        );
    }
    context.stack.clear();

    // 4. 64-bit integer comparisons
    println!("\n4. 64-bit Integer Comparisons:");

    // i64.eq (equal)
    context.push_comparison_value(Value::I64(0x123456789ABCDEF0))?;
    context.push_comparison_value(Value::I64(0x123456789ABCDEF0))?;
    println!("   Input: 0x123456789ABCDEF0, 0x123456789ABCDEF0");
    ComparisonOp::I64Eq.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Large i64 values equal: {} (true)", result);
    }
    context.stack.clear();

    // i64.lt_s (less than, signed)
    context.push_comparison_value(Value::I64(-9223372036854775808))?; // i64::MIN
    context.push_comparison_value(Value::I64(9223372036854775807))?; // i64::MAX
    println!("   Input: i64::MIN, i64::MAX");
    ComparisonOp::I64LtS.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   i64::MIN < i64::MAX (signed): {} (true)", result);
    }
    context.stack.clear();

    // i64.gt_u (greater than, unsigned)
    context.push_comparison_value(Value::I64(-1))?; // Large unsigned value
    context.push_comparison_value(Value::I64(1000))?;
    println!("   Input: -1 (large unsigned), 1000");
    ComparisonOp::I64GtU.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   -1 > 1000 (unsigned): {} (true)", result);
    }
    context.stack.clear();

    // 5. Float comparisons (f32)
    println!("\n5. Float Comparisons (f32):");

    // f32.eq (equal)
    context.push_comparison_value(Value::F32(FloatBits32::from_float(3.14159)))?;
    context.push_comparison_value(Value::F32(FloatBits32::from_float(3.14159)))?;
    println!("   Input: 3.14159, 3.14159");
    ComparisonOp::F32Eq.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   3.14159 == 3.14159: {} (true)", result);
    }
    context.stack.clear();

    // f32.lt (less than)
    context.push_comparison_value(Value::F32(FloatBits32::from_float(2.718)))?;
    context.push_comparison_value(Value::F32(FloatBits32::from_float(3.14159)))?;
    println!("   Input: 2.718, 3.14159");
    ComparisonOp::F32Lt.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   2.718 < 3.14159: {} (true)", result);
    }
    context.stack.clear();

    // f32.ge (greater than or equal)
    context.push_comparison_value(Value::F32(FloatBits32::from_float(5.0)))?;
    context.push_comparison_value(Value::F32(FloatBits32::from_float(5.0)))?;
    println!("   Input: 5.0, 5.0");
    ComparisonOp::F32Ge.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   5.0 >= 5.0: {} (true)", result);
    }
    context.stack.clear();

    // 6. Float comparisons (f64)
    println!("\n6. Float Comparisons (f64):");

    // f64.ne (not equal)
    context.push_comparison_value(Value::F64(FloatBits64::from_float(3.141592653589793)))?;
    context.push_comparison_value(Value::F64(FloatBits64::from_float(2.718281828459045)))?;
    println!("   Input: π (3.141592653589793), e (2.718281828459045)");
    ComparisonOp::F64Ne.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   π != e: {} (true)", result);
    }
    context.stack.clear();

    // f64.le (less than or equal)
    context.push_comparison_value(Value::F64(FloatBits64::from_float(1.414213562373095)))?; // sqrt(2)
    context.push_comparison_value(Value::F64(FloatBits64::from_float(1.732050807568877)))?; // sqrt(3)
    println!("   Input: sqrt(2) (1.414213562373095), sqrt(3) (1.732050807568877)");
    ComparisonOp::F64Le.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   sqrt(2) <= sqrt(3): {} (true)", result);
    }
    context.stack.clear();

    // 7. Test operations (eqz)
    println!("\n7. Test Operations (eqz - equals zero):");

    // i32.eqz with zero
    context.push_comparison_value(Value::I32(0))?;
    println!("   Input: 0");
    ComparisonOp::I32Eqz.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   0 == 0: {} (true)", result);
    }
    context.stack.clear();

    // i32.eqz with non-zero
    context.push_comparison_value(Value::I32(42))?;
    println!("   Input: 42");
    ComparisonOp::I32Eqz.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   42 == 0: {} (false)", result);
    }
    context.stack.clear();

    // i64.eqz with zero
    context.push_comparison_value(Value::I64(0))?;
    println!("   Input: 0i64");
    ComparisonOp::I64Eqz.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   0i64 == 0: {} (true)", result);
    }
    context.stack.clear();

    // i64.eqz with large non-zero
    context.push_comparison_value(Value::I64(0x123456789ABCDEF0))?;
    println!("   Input: 0x123456789ABCDEF0");
    ComparisonOp::I64Eqz.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   0x123456789ABCDEF0 == 0: {} (false)", result);
    }
    context.stack.clear();

    // 8. NaN handling in float comparisons
    println!("\n8. NaN Handling in Float Comparisons:");

    // f32 NaN == NaN (should be false)
    context.push_comparison_value(Value::F32(FloatBits32::from_float(f32::NAN)))?;
    context.push_comparison_value(Value::F32(FloatBits32::from_float(f32::NAN)))?;
    println!("   Input: NaN, NaN");
    ComparisonOp::F32Eq.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   NaN == NaN: {} (false - WebAssembly spec)", result);
    }
    context.stack.clear();

    // f32 NaN != anything (should be true)
    context.push_comparison_value(Value::F32(FloatBits32::from_float(f32::NAN)))?;
    context.push_comparison_value(Value::F32(FloatBits32::from_float(42.0)))?;
    println!("   Input: NaN, 42.0");
    ComparisonOp::F32Ne.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   NaN != 42.0: {} (true - WebAssembly spec)", result);
    }
    context.stack.clear();

    // f32 NaN < anything (should be false)
    context.push_comparison_value(Value::F32(FloatBits32::from_float(f32::NAN)))?;
    context.push_comparison_value(Value::F32(FloatBits32::from_float(42.0)))?;
    println!("   Input: NaN, 42.0");
    ComparisonOp::F32Lt.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!(
            "   NaN < 42.0: {} (false - NaN comparisons are always false)",
            result
        );
    }
    context.stack.clear();

    // f64 NaN != NaN (should be true)
    context.push_comparison_value(Value::F64(FloatBits64::from_float(f64::NAN)))?;
    context.push_comparison_value(Value::F64(FloatBits64::from_float(f64::NAN)))?;
    println!("   Input: NaN (f64), NaN (f64)");
    ComparisonOp::F64Ne.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   NaN != NaN (f64): {} (true - WebAssembly spec)", result);
    }
    context.stack.clear();

    // 9. Special float values
    println!("\n9. Special Float Values:");

    // Positive and negative infinity
    context.push_comparison_value(Value::F32(FloatBits32::from_float(f32::NEG_INFINITY)))?;
    context.push_comparison_value(Value::F32(FloatBits32::from_float(f32::INFINITY)))?;
    println!("   Input: -∞, +∞");
    ComparisonOp::F32Lt.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   -∞ c +∞: {} (true)", result);
    }
    context.stack.clear();

    // Positive and negative zero
    context.push_comparison_value(Value::F64(FloatBits64::from_float(-0.0)))?;
    context.push_comparison_value(Value::F64(FloatBits64::from_float(0.0)))?;
    println!("   Input: -0.0, +0.0");
    ComparisonOp::F64Eq.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   -0.0 == +0.0: {} (true - IEEE 754 spec)", result);
    }
    context.stack.clear();

    // 10. Edge cases and overflow scenarios
    println!("\n10. Edge Cases:");

    // Maximum i32 values
    context.push_comparison_value(Value::I32(i32::MAX))?;
    context.push_comparison_value(Value::I32(i32::MIN))?;
    println!("   Input: i32::MAX (2147483647), i32::MIN (-2147483648)");
    ComparisonOp::I32GtS.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   i32::MAX e i32::MIN (signed): {} (true)", result);
    }
    context.stack.clear();

    // Same values as unsigned comparison
    context.push_comparison_value(Value::I32(i32::MAX))?;
    context.push_comparison_value(Value::I32(i32::MIN))?;
    println!("   Input: i32::MAX (2147483647), i32::MIN (-2147483648 = 0x80000000)");
    ComparisonOp::I32LtU.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!(
            "   i32::MAX < i32::MIN (unsigned): {} (true - MIN as unsigned is 2^31)",
            result
        );
    }

    println!("\n=== Demo Complete ===");
    println!("\nKey Takeaways:");
    println!("- All comparison operations return i32 values (0 for false, 1 for true)");
    println!("- Signed vs unsigned comparisons can produce different results");
    println!("- NaN handling follows WebAssembly specification exactly");
    println!("- Float comparisons handle special values (±∞, ±0, NaN) correctly");
    println!("- Integer operations work with full 32-bit and 64-bit ranges");

    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() {
    // Binary std/no_std choice - ASIL-D safe: exit gracefully
    eprintln!("This example requires std or alloc features");
    core::process::exit(1);
}
