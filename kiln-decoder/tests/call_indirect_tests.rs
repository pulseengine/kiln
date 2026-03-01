// These tests reference kiln_decoder::instructions::{Instruction, encode_instruction, parse_instruction}
// which is an API that has not been implemented in kiln-decoder yet.
// The instruction parsing API currently lives in kiln-runtime::instruction_parser with a different
// signature (parse_instructions parses full bytecode sequences, not individual instructions).
//
// These tests are disabled until a single-instruction parse/encode API is added to kiln-decoder.

#[test]
#[ignore = "kiln_decoder::instructions API not yet implemented"]
fn test_parse_encode_call_indirect_basic() {
    // call_indirect (type_idx=1, table_idx=0)
    // Requires kiln_decoder::instructions::{parse_instruction, encode_instruction, Instruction}
}

#[test]
#[ignore = "kiln_decoder::instructions API not yet implemented"]
fn test_parse_encode_call_indirect_larger_type_idx() {
    // call_indirect (type_idx=128, table_idx=0)
    // Requires kiln_decoder::instructions::{parse_instruction, encode_instruction, Instruction}
}

#[test]
#[ignore = "kiln_decoder::instructions API not yet implemented"]
fn test_parse_encode_call_indirect_nonzero_table() {
    // Non-zero table index for future-compatibility
    // Requires kiln_decoder::instructions::{parse_instruction, encode_instruction, Instruction}
}

#[test]
#[ignore = "kiln_decoder::instructions API not yet implemented"]
fn test_parse_call_indirect_invalid() {
    // Missing table index byte - should return error
    // Requires kiln_decoder::instructions::parse_instruction
}
