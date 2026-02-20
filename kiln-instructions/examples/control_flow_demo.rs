//! Demonstration of WebAssembly control flow operations
//!
//! This example shows how to use:
//! - Return instruction
//! - Call indirect instruction
//! - Branch table instruction

#[cfg(feature = "std")]
use std::vec::Vec;
use std::vec::Vec;

use wrt_error::Result;
use wrt_foundation::{
    FloatBits32,
    Value,
};
use wrt_instructions::{
    Block,
    BrTable,
    CallIndirect,
    ControlContext,
    ControlOp,
    FunctionOperations,
    PureInstruction,
    Return,
};

// Mock execution context for demonstration
#[cfg(feature = "std")]
struct DemoContext {
    stack:           Vec<Value>,
    returned:        bool,
    called_function: Option<u32>,
    indirect_call:   Option<(u32, u32)>,
    branch_target:   Option<u32>,
}

#[cfg(feature = "std")]
impl DemoContext {
    fn new() -> Self {
        Self {
            stack:           Vec::new(),
            returned:        false,
            called_function: None,
            indirect_call:   None,
            branch_target:   None,
        }
    }
}

#[cfg(feature = "std")]
impl ControlContext for DemoContext {
    fn push_control_value(&mut self, value: Value) -> Result<()> {
        self.stack.push(value);
        Ok(())
    }

    fn pop_control_value(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| wrt_error::Error::runtime_error("Stack underflow"))
    }

    fn get_block_depth(&self) -> usize {
        0 // Simplified for demo
    }

    fn enter_block(&mut self, _block_type: Block) -> Result<()> {
        Ok(())
    }

    fn exit_block(&mut self) -> Result<Block> {
        Ok(Block::Block(wrt_foundation::BlockType::Value(None)))
    }

    fn branch(&mut self, target: wrt_instructions::BranchTarget) -> Result<()> {
        self.branch_target = Some(target.label_idx);
        Ok(())
    }

    fn return_function(&mut self) -> Result<()> {
        self.returned = true;
        Ok(())
    }

    fn call_function(&mut self, func_idx: u32) -> Result<()> {
        self.called_function = Some(func_idx);
        Ok(())
    }

    fn call_indirect(&mut self, table_idx: u32, type_idx: u32) -> Result<()> {
        self.indirect_call = Some((table_idx, type_idx));
        Ok(())
    }

    fn trap(&mut self, _message: &str) -> Result<()> {
        Err(wrt_error::Error::runtime_error("Trap"))
    }

    fn get_current_block(&self) -> Option<&Block> {
        None
    }

    fn get_function_operations(&mut self) -> Result<&mut dyn FunctionOperations> {
        Ok(self as &mut dyn FunctionOperations)
    }

    fn execute_return(&mut self) -> Result<()> {
        self.returned = true;
        Ok(())
    }

    fn execute_call_indirect(
        &mut self,
        table_idx: u32,
        type_idx: u32,
        func_idx: i32,
    ) -> Result<()> {
        if func_idx < 0 {
            return Err(wrt_error::Error::runtime_error("Invalid function index"));
        }

        // Validate and execute indirect call
        self.indirect_call = Some((table_idx, type_idx));
        Ok(())
    }

    fn execute_br_table(&mut self, table: &[u32], default: u32, index: i32) -> Result<()> {
        let label_idx = if index >= 0 && (index as usize) < table.len() {
            table[index as usize]
        } else {
            default
        };

        self.branch_target = Some(label_idx);
        Ok(())
    }
}

#[cfg(feature = "std")]
impl FunctionOperations for DemoContext {
    fn get_function_type(&self, func_idx: u32) -> Result<u32> {
        // Mock: return type index based on function index
        Ok(func_idx % 5) // 5 different function types
    }

    fn get_table_function(&self, table_idx: u32, elem_idx: u32) -> Result<u32> {
        // Mock: simple function index calculation
        Ok(table_idx * 100 + elem_idx)
    }

    fn validate_function_signature(&self, func_idx: u32, expected_type: u32) -> Result<()> {
        let actual_type = self.get_function_type(func_idx)?;
        if actual_type == expected_type {
            Ok(())
        } else {
            Err(wrt_error::Error::type_error("Function signature mismatch"))
        }
    }

    fn execute_function_call(&mut self, func_idx: u32) -> Result<()> {
        self.called_function = Some(func_idx);
        Ok(())
    }
}

#[cfg(feature = "std")]
fn main() -> Result<()> {
    println!("=== WebAssembly Control Flow Operations Demo ===\n");

    let mut context = DemoContext::new();

    // 1. Demonstrate Return instruction
    println!("1. Return Operation:");
    let return_op = Return::new();
    return_op.execute(&mut context)?;
    println!("   Executed return instruction");
    println!("   Function returned: {}", context.returned);

    // Reset context for next demo
    context.returned = false;

    // 2. Demonstrate CallIndirect instruction
    println!("\n2. Call Indirect Operation:");
    // Push function index onto stack
    context.push_control_value(Value::I32(42))?;

    let call_indirect = CallIndirect::new(0, 2); // table 0, type 2
    call_indirect.execute(&mut context)?;
    println!("   Executed call_indirect with table=0, type=2, func_index=42");
    println!("   Indirect call executed: {:?}", context.indirect_call);

    // Reset context for next demo
    context.indirect_call = None;

    // 3. Demonstrate BrTable instruction
    println!("\n3. Branch Table Operation:");

    // Test with in-range index
    context.push_control_value(Value::I32(1))?; // Index 1
    let br_table = BrTable::from_slice(&[10, 20, 30], 99)?;
    br_table.execute(&mut context)?;
    println!("   Executed br_table with index=1, table=[10,20,30], default=99");
    println!("   Branched to label: {:?}", context.branch_target);

    // Reset and test with out-of-range index
    context.branch_target = None;
    context.push_control_value(Value::I32(5))?; // Out of range
    let br_table = BrTable::from_slice(&[10, 20, 30], 99)?;
    br_table.execute(&mut context)?;
    println!("   Executed br_table with index=5 (out of range)");
    println!("   Branched to default label: {:?}", context.branch_target);

    // 4. Demonstrate unified ControlOp enum
    println!("\n4. Unified Control Operations:");

    // Test Return through ControlOp
    let control_return = ControlOp::Return;
    context.returned = false;
    control_return.execute(&mut context)?;
    println!("   ControlOp::Return executed: {}", context.returned);

    // Test CallIndirect through ControlOp
    context.push_control_value(Value::I32(7))?;
    let control_call_indirect = ControlOp::CallIndirect {
        table_idx: 1,
        type_idx:  3,
    };
    context.indirect_call = None;
    control_call_indirect.execute(&mut context)?;
    println!(
        "   ControlOp::CallIndirect executed: {:?}",
        context.indirect_call
    );

    // Binary std/no_std choice
    #[cfg(feature = "std")]
    {
        context.push_control_value(Value::I32(0))?;
        let control_br_table = ControlOp::BrTable {
            table:   vec![100, 200, 300],
            default: 999,
        };

        context.branch_target = None;
        control_br_table.execute(&mut context)?;
        println!(
            "   ControlOp::BrTable executed: {:?}",
            context.branch_target
        );
    }

    #[cfg(not(feature = "std"))]
    println!("   ControlOp::BrTable test skipped (requires alloc)");

    // 5. Demonstrate error handling
    println!("\n5. Error Handling:");

    // Test CallIndirect with negative function index
    context.push_control_value(Value::I32(-1))?;
    let invalid_call = CallIndirect::new(0, 1);
    match invalid_call.execute(&mut context) {
        Ok(_) => println!("   Unexpected success with negative function index"),
        Err(e) => println!("   Expected error with negative function index: {}", e),
    }

    // Test type validation
    context.push_control_value(Value::F32(FloatBits32::from_float(3.14)))?; // Wrong type
    let type_error_call = CallIndirect::new(0, 1);
    match type_error_call.execute(&mut context) {
        Ok(_) => println!("   Unexpected success with wrong type"),
        Err(e) => println!("   Expected type error: {}", e),
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() {
    // Binary std/no_std choice
    eprintln!("This example requires std or alloc features");
}
