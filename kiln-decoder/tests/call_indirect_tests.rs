use wrt_decoder::instructions::{Instruction, encode_instruction, parse_instruction};
use wrt_format::binary;

#[test]
fn test_parse_encode_call_indirect_basic() {
    // call_indirect (type_idx=1, table_idx=0)
    let bytes = vec![binary::CALL_INDIRECT, 0x01, 0x00];
    let (instruction, bytes_read) = parse_instruction(&bytes).unwrap();

    assert_eq!(instruction, Instruction::CallIndirect(1, 0));
    assert_eq!(bytes_read, 3);

    let encoded = encode_instruction(&instruction).unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn test_parse_encode_call_indirect_larger_type_idx() {
    // call_indirect (type_idx=128, table_idx=0)
    // 128 in LEB128 is [0x80, 0x01]
    let bytes = vec![binary::CALL_INDIRECT, 0x80, 0x01, 0x00];
    let (instruction, bytes_read) = parse_instruction(&bytes).unwrap();

    assert_eq!(instruction, Instruction::CallIndirect(128, 0));
    assert_eq!(bytes_read, 4);

    let encoded = encode_instruction(&instruction).unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn test_parse_encode_call_indirect_nonzero_table() {
    // This test uses a non-zero table index, which is not valid in MVP
    // but the parser should handle it for future-compatibility
    let bytes = vec![binary::CALL_INDIRECT, 0x05, 0x01];
    let (instruction, bytes_read) = parse_instruction(&bytes).unwrap();

    assert_eq!(instruction, Instruction::CallIndirect(5, 1));
    assert_eq!(bytes_read, 3);

    let encoded = encode_instruction(&instruction).unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn test_parse_call_indirect_invalid() {
    // Missing table index byte
    let bytes = vec![binary::CALL_INDIRECT, 0x05];
    let result = parse_instruction(&bytes);

    assert!(result.is_err());
}
