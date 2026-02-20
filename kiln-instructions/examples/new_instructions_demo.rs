//! Demonstration of the newly added WebAssembly instructions
//!
//! This example shows how to use:
//! - Parametric operations (drop, select)
//! - Memory operations (size, grow)
//! - Test operations (i32.eqz, i64.eqz)

use wrt_error::Result;
use wrt_foundation::Value;
use wrt_instructions::{
    comparison_ops::ComparisonContext,
    parametric_ops::ParametricContext,
    ComparisonOp,
    ParametricOp,
    PureInstruction,
};

// Mock contexts for demonstration
struct SimpleContext {
    stack: Vec<Value>,
}

impl SimpleContext {
    fn new() -> Self {
        Self { stack: Vec::new() }
    }
}

// Implement ParametricContext
impl ParametricContext for SimpleContext {
    fn push_value(&mut self, value: Value) -> Result<()> {
        self.stack.push(value);
        Ok(())
    }

    fn pop_value(&mut self) -> Result<Value> {
        self.stack.pop().ok_or_else(|| {
            wrt_error::Error::runtime_execution_error("Stack underflow")
        })
    }

    fn peek_value(&self) -> Result<&Value> {
        self.stack.last().ok_or_else(|| {
            wrt_error::Error::new(
                wrt_error::ErrorCategory::Runtime,
                wrt_error::codes::STACK_UNDERFLOW,
                "Stack underflow",
            )
        })
    }
}

// Implement ComparisonContext
impl ComparisonContext for SimpleContext {
    fn pop_comparison_value(&mut self) -> Result<Value> {
        self.pop_value()
    }

    fn push_comparison_value(&mut self, value: Value) -> Result<()> {
        self.push_value(value)
    }
}

fn main() -> Result<()> {
    println!("=== New WebAssembly Instructions Demo ===\n");

    // 1. Demonstrate DROP operation
    println!("1. DROP Operation:");
    let mut ctx = SimpleContext::new();
    ctx.push_value(Value::I32(42))?;
    println!("   Stack before drop: {:?}", ctx.stack);
    ParametricOp::Drop.execute(&mut ctx)?;
    println!("   Stack after drop: {:?}", ctx.stack);

    // 2. Demonstrate SELECT operation
    println!("\n2. SELECT Operation:");
    ctx.push_value(Value::I32(10))?; // first option
    ctx.push_value(Value::I32(20))?; // second option
    ctx.push_value(Value::I32(1))?; // condition (true)
    println!("   Stack before select: {:?}", ctx.stack);
    ParametricOp::Select.execute(&mut ctx)?;
    println!("   Result (selected first): {:?}", ctx.pop_value()?);

    // 3. Demonstrate I32.EQZ operation
    println!("\n3. I32.EQZ Operation:");
    ctx.push_value(Value::I32(0))?;
    println!("   Testing if 0 == 0: ");
    ComparisonOp::I32Eqz.execute(&mut ctx)?;
    println!("   Result: {:?} (1 means true)", ctx.pop_value()?);

    ctx.push_value(Value::I32(42))?;
    println!("   Testing if 42 == 0: ");
    ComparisonOp::I32Eqz.execute(&mut ctx)?;
    println!("   Result: {:?} (0 means false)", ctx.pop_value()?);

    // 4. Demonstrate I64.EQZ operation
    println!("\n4. I64.EQZ Operation:");
    ctx.push_value(Value::I64(0))?;
    println!("   Testing if 0i64 == 0: ");
    ComparisonOp::I64Eqz.execute(&mut ctx)?;
    println!("   Result: {:?} (1 means true)", ctx.pop_value()?);

    // Note: Memory operations would require a proper memory implementation
    println!("\n5. Memory Operations (MemorySize, MemoryGrow):");
    println!("   These require a WebAssembly memory instance to demonstrate.");
    println!("   - memory.size returns current size in pages");
    println!("   - memory.grow attempts to grow memory and returns previous size");

    println!("\n=== Demo Complete ===");
    Ok(())
}
