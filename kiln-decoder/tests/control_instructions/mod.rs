use wrt_decoder::instructions::{Instruction, encode_instruction, parse_instruction};
use wrt_decoder::types::BlockType;
use wrt_foundation::ValueType;

#[test]
fn test_parse_encode_block() {
    // block (empty) end
    let bytes = vec![0x02, 0x40, 0x0B];
    let (instruction, bytes_read) = parse_instruction(&bytes).unwrap();
    
    match instruction {
        Instruction::Block(block_type, instructions) => {
            assert_eq!(block_type, BlockType::Empty;
            assert!(instructions.is_empty());
        }
        _ => panic!("Expected Block instruction"),
    }
    
    assert_eq!(bytes_read, 3;
    
    let encoded = encode_instruction(&instruction).unwrap();
    assert_eq!(encoded, bytes;
}

#[test]
fn test_parse_encode_loop() {
    // loop (i32) i32.const 1 end
    let bytes = vec![
        0x03, 0x7F, // loop with i32 return type
        0x41, 0x01, // i32.const 1
        0x0B        // end
    ];
    let (instruction, bytes_read) = parse_instruction(&bytes).unwrap();
    
    match instruction {
        Instruction::Loop(block_type, instructions) => {
            assert_eq!(block_type, BlockType::Value(ValueType::I32;
            assert_eq!(instructions.len(), 1);
            assert_eq!(instructions[0], Instruction::I32Const(1;
        }
        _ => panic!("Expected Loop instruction"),
    }
    
    assert_eq!(bytes_read, 5;
    
    let encoded = encode_instruction(&instruction).unwrap();
    assert_eq!(encoded, bytes;
}

#[test]
fn test_parse_encode_if() {
    // if (empty) i32.const 1 else i32.const 0 end
    let bytes = vec![
        0x04, 0x40, // if with empty block type
        0x41, 0x01, // i32.const 1
        0x05,       // else
        0x41, 0x00, // i32.const 0
        0x0B        // end
    ];
    let (instruction, bytes_read) = parse_instruction(&bytes).unwrap();
    
    match instruction {
        Instruction::If(block_type, then_instructions, else_instructions) => {
            assert_eq!(block_type, BlockType::Empty;
            assert_eq!(then_instructions.len(), 1);
            assert_eq!(then_instructions[0], Instruction::I32Const(1;
            assert_eq!(else_instructions.len(), 1);
            assert_eq!(else_instructions[0], Instruction::I32Const(0;
        }
        _ => panic!("Expected If instruction"),
    }
    
    assert_eq!(bytes_read, 8;
    
    let encoded = encode_instruction(&instruction).unwrap();
    assert_eq!(encoded, bytes;
}

#[test]
fn test_parse_encode_br_table() {
    // br_table [0 1] 2
    let bytes = vec![
        0x0E,       // br_table
        0x02,       // count = 2
        0x00,       // label 0
        0x01,       // label 1
        0x02,       // default label 2
    ];
    let (instruction, bytes_read) = parse_instruction(&bytes).unwrap();
    
    match instruction {
        Instruction::BrTable(labels, default_label) => {
            assert_eq!(labels, vec![0, 1];
            assert_eq!(default_label, 2;
        }
        _ => panic!("Expected BrTable instruction"),
    }
    
    assert_eq!(bytes_read, 5;
    
    let encoded = encode_instruction(&instruction).unwrap();
    assert_eq!(encoded, bytes;
}

#[test]
fn test_nested_blocks() {
    // block block end end
    let bytes = vec![
        0x02, 0x40, // outer block
        0x02, 0x40, // inner block
        0x0B,       // inner end
        0x0B        // outer end
    ];
    let (instruction, bytes_read) = parse_instruction(&bytes).unwrap();
    
    match &instruction {
        Instruction::Block(block_type, instructions) => {
            assert_eq!(*block_type, BlockType::Empty;
            assert_eq!(instructions.len(), 1);
            
            match &instructions[0] {
                Instruction::Block(inner_block_type, inner_instructions) => {
                    assert_eq!(*inner_block_type, BlockType::Empty;
                    assert!(inner_instructions.is_empty());
                }
                _ => panic!("Expected inner Block instruction"),
            }
        }
        _ => panic!("Expected outer Block instruction"),
    }
    
    assert_eq!(bytes_read, 6;
    
    let encoded = encode_instruction(&instruction).unwrap();
    assert_eq!(encoded, bytes;
} 