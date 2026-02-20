//! Demonstration of WebAssembly conversion operations
//!
//! This example shows how to use:
//! - Integer conversions (wrap, extend, truncate)
//! - Float conversions (convert, promote, demote)
//! - Reinterpret operations
//! - Saturating truncations

use std::vec::Vec;

use wrt_error::Result;
use wrt_foundation::{
    FloatBits32,
    FloatBits64,
    Value,
};
use wrt_instructions::{
    ConversionContext,
    ConversionOp,
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
impl ConversionContext for DemoContext {
    fn pop_conversion_value(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| wrt_error::Error::runtime_error("Stack underflow"))
    }

    fn push_conversion_value(&mut self, value: Value) -> Result<()> {
        self.stack.push(value);
        Ok(())
    }
}

#[cfg(feature = "std")]
fn main() -> Result<()> {
    println!("=== WebAssembly Conversion Operations Demo ===\n");

    let mut context = DemoContext::new();

    // 1. Integer wrapping (i32.wrap_i64)
    println!("1. Integer Wrapping (i32.wrap_i64):");
    context.push_conversion_value(Value::I64(0x1234567890ABCDEF))?;
    println!("   Input: i64 = 0x{:016X}", 0x1234567890ABCDEF_i64);
    ConversionOp::I32WrapI64.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Result: i32 = 0x{:08X} (lower 32 bits)", result);
    }
    context.stack.clear();

    // 2. Integer sign extension (i64.extend_i32_s)
    println!("\n2. Sign Extension (i64.extend_i32_s):");
    context.push_conversion_value(Value::I32(-42))?;
    println!("   Input: i32 = -42");
    ConversionOp::I64ExtendI32S.execute(&mut context)?;
    if let Some(Value::I64(result)) = context.peek() {
        println!("   Result: i64 = {} (sign extended)", result);
    }
    context.stack.clear();

    // 3. Integer zero extension (i64.extend_i32_u)
    println!("\n3. Zero Extension (i64.extend_i32_u):");
    context.push_conversion_value(Value::I32(-1))?; // 0xFFFFFFFF as u32
    println!("   Input: i32 = -1 (0xFFFFFFFF as u32)");
    ConversionOp::I64ExtendI32U.execute(&mut context)?;
    if let Some(Value::I64(result)) = context.peek() {
        println!("   Result: i64 = {} (zero extended)", result);
    }
    context.stack.clear();

    // 4. Float to integer conversion with trapping (i32.trunc_f32_s)
    println!("\n4. Float to Integer Truncation (i32.trunc_f32_s):");
    context.push_conversion_value(Value::F32(FloatBits32::from_float(42.7)))?;
    println!("   Input: f32 = 42.7");
    ConversionOp::I32TruncF32S.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Result: i32 = {} (truncated)", result);
    }
    context.stack.clear();

    // 5. Integer to float conversion (f32.convert_i32_s)
    println!("\n5. Integer to Float Conversion (f32.convert_i32_s):");
    context.push_conversion_value(Value::I32(-100))?;
    println!("   Input: i32 = -100");
    ConversionOp::F32ConvertI32S.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!("   Result: f32 = {}", result.value());
    }
    context.stack.clear();

    // 6. Float promotion (f64.promote_f32)
    println!("\n6. Float Promotion (f64.promote_f32):");
    context.push_conversion_value(Value::F32(FloatBits32::from_float(3.14159)))?;
    println!("   Input: f32 = 3.14159");
    ConversionOp::F64PromoteF32.execute(&mut context)?;
    if let Some(Value::F64(result)) = context.peek() {
        println!("   Result: f64 = {} (promoted)", result.value());
    }
    context.stack.clear();

    // 7. Float demotion (f32.demote_f64)
    println!("\n7. Float Demotion (f32.demote_f64):");
    context.push_conversion_value(Value::F64(FloatBits64::from_float(3.141592653589793)))?;
    println!("   Input: f64 = 3.141592653589793");
    ConversionOp::F32DemoteF64.execute(&mut context)?;
    if let Some(Value::F32(result)) = context.peek() {
        println!(
            "   Result: f32 = {} (demoted, precision lost)",
            result.value()
        );
    }
    context.stack.clear();

    // 8. Reinterpret operations (i32.reinterpret_f32)
    println!("\n8. Reinterpret Operations (i32.reinterpret_f32):");
    let float_val = FloatBits32::from_float(1.0);
    context.push_conversion_value(Value::F32(float_val))?;
    println!("   Input: f32 = 1.0 (bits: 0x{:08X})", float_val.0);
    ConversionOp::I32ReinterpretF32.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Result: i32 = 0x{:08X} (same bit pattern)", result);
    }
    context.stack.clear();

    // 9. Saturating truncation (i32.trunc_sat_f32_s)
    println!("\n9. Saturating Truncation (i32.trunc_sat_f32_s):");

    // Test with a very large value
    context.push_conversion_value(Value::F32(FloatBits32::from_float(1e10)))?;
    println!("   Input: f32 = 1e10 (out of i32 range)");
    ConversionOp::I32TruncSatF32S.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Result: i32 = {} (saturated to i32::MAX)", result);
    }
    context.stack.clear();

    // Test with NaN
    context.push_conversion_value(Value::F32(FloatBits32::from_float(f32::NAN)))?;
    println!("   Input: f32 = NaN");
    ConversionOp::I32TruncSatF32S.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!("   Result: i32 = {} (NaN converts to 0)", result);
    }
    context.stack.clear();

    // 10. Sign extension operations
    println!("\n10. Sign Extension Operations:");

    // i32.extend8_s
    context.push_conversion_value(Value::I32(0xFF))?; // -1 as i8
    println!("   Input: i32 = 0xFF (255, or -1 as i8)");
    ConversionOp::I32Extend8S.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!(
            "   Result after i32.extend8_s: {} (sign extended from 8 bits)",
            result
        );
    }
    context.stack.clear();

    // i32.extend16_s
    context.push_conversion_value(Value::I32(0x8000))?; // -32768 as i16
    println!("   Input: i32 = 0x8000 (32768, or -32768 as i16)");
    ConversionOp::I32Extend16S.execute(&mut context)?;
    if let Some(Value::I32(result)) = context.peek() {
        println!(
            "   Result after i32.extend16_s: {} (sign extended from 16 bits)",
            result
        );
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() {
    // Binary std/no_std choice - ASIL-D safe: exit gracefully
    eprintln!("This example requires std or alloc features");
    core::process::exit(1);
}
