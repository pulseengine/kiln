//! Tests for the instruction parser module
//!
//! This module tests the bytecode parsing functionality that converts
//! raw WebAssembly bytecode into typed instructions.

#[cfg(test)]
mod tests {
    use wrt_foundation::types::Instruction;

    use crate::instruction_parser::parse_instructions;

    #[test]
    fn test_parse_nop() {
        let bytecode = vec![0x01, 0x0B]; // nop + end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 2);
        assert!(matches!(instructions.get(0).unwrap(), Instruction::Nop));
    }

    #[test]
    fn test_parse_unreachable() {
        let bytecode = vec![0x00, 0x0B]; // unreachable + end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 2);
        assert!(matches!(instructions.get(0).unwrap(), Instruction::Unreachable));
    }

    #[test]
    fn test_parse_i32_const() {
        // 127 in signed LEB128 requires 2 bytes: 0xFF 0x00
        // (single byte 0x7F = -1 in signed LEB128)
        let bytecode = vec![0x41, 0xFF, 0x00, 0x0B]; // i32.const 127 + end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 2);
        match instructions.get(0).unwrap() {
            Instruction::I32Const(val) => assert_eq!(*val, 127),
            _ => panic!("Expected I32Const instruction"),
        }
    }

    #[test]
    fn test_parse_i64_const() {
        // 100 in signed LEB128 requires 2 bytes: 0xE4 0x00
        // (single byte 0x64 would be sign-extended to negative)
        let bytecode = vec![0x42, 0xE4, 0x00, 0x0B]; // i64.const 100 + end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 2);
        match instructions.get(0).unwrap() {
            Instruction::I64Const(val) => assert_eq!(*val, 100),
            _ => panic!("Expected I64Const instruction"),
        }
    }

    #[test]
    fn test_parse_local_get() {
        let bytecode = vec![0x20, 0x02, 0x0B]; // local.get 2 + end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 2);
        match instructions.get(0).unwrap() {
            Instruction::LocalGet(idx) => assert_eq!(*idx, 2),
            _ => panic!("Expected LocalGet instruction"),
        }
    }

    #[test]
    fn test_parse_local_set() {
        let bytecode = vec![0x21, 0x03, 0x0B]; // local.set 3 + end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 2);
        match instructions.get(0).unwrap() {
            Instruction::LocalSet(idx) => assert_eq!(*idx, 3),
            _ => panic!("Expected LocalSet instruction"),
        }
    }

    #[test]
    fn test_parse_i32_add() {
        let bytecode = vec![0x6A, 0x0B]; // i32.add + end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 2);
        assert!(matches!(instructions.get(0).unwrap(), Instruction::I32Add));
    }

    #[test]
    fn test_parse_return() {
        let bytecode = vec![0x0F, 0x0B]; // return + end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 2);
        assert!(matches!(instructions.get(0).unwrap(), Instruction::Return));
    }

    #[test]
    fn test_parse_end() {
        let bytecode = vec![0x0B]; // end
        let instructions = parse_instructions(&bytecode).unwrap();
        assert_eq!(instructions.len(), 1);
        assert!(matches!(instructions.get(0).unwrap(), Instruction::End));
    }

    #[test]
    fn test_parse_unknown_opcode() {
        let bytecode = vec![0xFF]; // Invalid opcode
        let result = parse_instructions(&bytecode);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_bytecode() {
        let bytecode = vec![];
        let result = parse_instructions(&bytecode);
        // Empty bytecode should fail since we need at least an End instruction
        assert!(result.is_err());
    }
}
