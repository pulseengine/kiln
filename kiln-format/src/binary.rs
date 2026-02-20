// WebAssembly binary format utilities
//
// This module provides utilities for working with the WebAssembly binary
// format.

// Core modules
use core::str;
#[cfg(all(feature = "std", feature = "safety-critical"))]
use std::{format, string::String};
#[cfg(all(feature = "std", not(feature = "safety-critical")))]
use std::{format, string::String, vec::Vec};

#[cfg(feature = "std")]
use wrt_error::{Error, ErrorCategory, Result, codes};
// Conditional imports for different environments
#[cfg(all(feature = "std", feature = "safety-critical"))]
use wrt_foundation::allocator::{CrateId, WrtVec};
#[cfg(not(feature = "std"))]
use wrt_foundation::bounded::{BoundedString, BoundedVec};
// wrt_error is imported above unconditionally
#[cfg(feature = "std")]
use wrt_foundation::{RefType, ValueType};

use crate::error::parse_error;
#[cfg(feature = "std")]
use crate::module::{Element, ElementInit, Module};
#[cfg(feature = "std")]
use crate::pure_format_types::{
    PureDataMode, PureDataSegment, PureElementInit, PureElementMode, PureElementSegment,
};
#[cfg(feature = "std")]
use crate::types::FormatBlockType;

/// Magic bytes for WebAssembly modules: \0asm
pub const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// WebAssembly binary format version
pub const WASM_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

/// WebAssembly section IDs
pub const CUSTOM_SECTION_ID: u8 = 0x00;
pub const TYPE_SECTION_ID: u8 = 0x01;
pub const IMPORT_SECTION_ID: u8 = 0x02;
pub const FUNCTION_SECTION_ID: u8 = 0x03;
pub const TABLE_SECTION_ID: u8 = 0x04;
pub const MEMORY_SECTION_ID: u8 = 0x05;
pub const GLOBAL_SECTION_ID: u8 = 0x06;
pub const EXPORT_SECTION_ID: u8 = 0x07;
pub const START_SECTION_ID: u8 = 0x08;
pub const ELEMENT_SECTION_ID: u8 = 0x09;
pub const CODE_SECTION_ID: u8 = 0x0A;
pub const DATA_SECTION_ID: u8 = 0x0B;
pub const DATA_COUNT_SECTION_ID: u8 = 0x0C;
pub const TAG_SECTION_ID: u8 = 0x0D;

/// WebAssembly value types
pub const I32_TYPE: u8 = 0x7F;
pub const I64_TYPE: u8 = 0x7E;
pub const F32_TYPE: u8 = 0x7D;
pub const F64_TYPE: u8 = 0x7C;
pub const V128_TYPE: u8 = 0x7B; // For SIMD extension
pub const FUNCREF_TYPE: u8 = 0x70;
pub const EXTERNREF_TYPE: u8 = 0x6F;
pub const EXNREF_TYPE: u8 = 0x69;

/// WebAssembly GC heap type opcodes (GC proposal)
pub const HEAP_TYPE_FUNC: u8 = 0x70; // func
pub const HEAP_TYPE_EXTERN: u8 = 0x6F; // extern
pub const HEAP_TYPE_ANY: u8 = 0x6E; // any (GC)
pub const HEAP_TYPE_EQ: u8 = 0x6D; // eq (GC)
pub const HEAP_TYPE_I31: u8 = 0x6C; // i31 (GC)
pub const HEAP_TYPE_STRUCT: u8 = 0x6B; // struct (GC)
pub const HEAP_TYPE_ARRAY: u8 = 0x6A; // array (GC)
pub const HEAP_TYPE_EXN: u8 = 0x69; // exn (exception handling)
pub const HEAP_TYPE_NOFUNC: u8 = 0x73; // nofunc (bottom type for func)
pub const HEAP_TYPE_NOEXTERN: u8 = 0x72; // noextern (bottom type for extern)
pub const HEAP_TYPE_NONE: u8 = 0x71; // none (bottom type for any)

/// WebAssembly GC reference type prefixes
pub const REF_TYPE_NON_NULLABLE: u8 = 0x64; // ref ht (non-nullable)
pub const REF_TYPE_NULLABLE: u8 = 0x63; // ref null ht (nullable)

/// WebAssembly GC composite type markers
pub const COMPOSITE_TYPE_ARRAY: u8 = 0x5E; // array field
pub const COMPOSITE_TYPE_STRUCT: u8 = 0x5F; // struct fields
pub const COMPOSITE_TYPE_FUNC: u8 = 0x60; // func params results
pub const COMPOSITE_TYPE_REC: u8 = 0x4E; // rec types
pub const COMPOSITE_TYPE_SUB_FINAL: u8 = 0x4F; // sub final supertypes
pub const COMPOSITE_TYPE_SUB: u8 = 0x50; // sub supertypes

/// WebAssembly GC field mutability
pub const FIELD_MUTABLE: u8 = 0x01;
pub const FIELD_IMMUTABLE: u8 = 0x00;

/// FB-prefixed opcodes (GC proposal)
pub const PREFIX_FB: u8 = 0xFB;
// GC struct instructions
pub const GC_STRUCT_NEW: u8 = 0x00;
pub const GC_STRUCT_NEW_DEFAULT: u8 = 0x01;
pub const GC_STRUCT_GET: u8 = 0x02;
pub const GC_STRUCT_GET_S: u8 = 0x03;
pub const GC_STRUCT_GET_U: u8 = 0x04;
pub const GC_STRUCT_SET: u8 = 0x05;
// GC array instructions
pub const GC_ARRAY_NEW: u8 = 0x06;
pub const GC_ARRAY_NEW_DEFAULT: u8 = 0x07;
pub const GC_ARRAY_NEW_FIXED: u8 = 0x08;
pub const GC_ARRAY_NEW_DATA: u8 = 0x09;
pub const GC_ARRAY_NEW_ELEM: u8 = 0x0A;
pub const GC_ARRAY_GET: u8 = 0x0B;
pub const GC_ARRAY_GET_S: u8 = 0x0C;
pub const GC_ARRAY_GET_U: u8 = 0x0D;
pub const GC_ARRAY_SET: u8 = 0x0E;
pub const GC_ARRAY_LEN: u8 = 0x0F;
pub const GC_ARRAY_FILL: u8 = 0x10;
pub const GC_ARRAY_COPY: u8 = 0x11;
pub const GC_ARRAY_INIT_DATA: u8 = 0x12;
pub const GC_ARRAY_INIT_ELEM: u8 = 0x13;
// GC reference type testing/casting
pub const GC_REF_TEST: u8 = 0x14;
pub const GC_REF_TEST_NULL: u8 = 0x15;
pub const GC_REF_CAST: u8 = 0x16;
pub const GC_REF_CAST_NULL: u8 = 0x17;
// GC branching
pub const GC_BR_ON_CAST: u8 = 0x18;
pub const GC_BR_ON_CAST_FAIL: u8 = 0x19;
// GC any/extern conversions
pub const GC_ANY_CONVERT_EXTERN: u8 = 0x1A;
pub const GC_EXTERN_CONVERT_ANY: u8 = 0x1B;
// i31 operations
pub const GC_REF_I31: u8 = 0x1C;
pub const GC_I31_GET_S: u8 = 0x1D;
pub const GC_I31_GET_U: u8 = 0x1E;

/// WebAssembly control instructions
pub const UNREACHABLE: u8 = 0x00;
pub const NOP: u8 = 0x01;
pub const BLOCK: u8 = 0x02;
pub const LOOP: u8 = 0x03;
pub const IF: u8 = 0x04;
pub const ELSE: u8 = 0x05;
// Exception handling opcodes (0x06-0x0A reserved for EH proposal)
pub const TRY: u8 = 0x06;
pub const CATCH: u8 = 0x07;
pub const THROW: u8 = 0x08;
pub const RETHROW: u8 = 0x09;
pub const THROW_REF: u8 = 0x0A;
pub const END: u8 = 0x0B;
pub const BR: u8 = 0x0C;
pub const BR_IF: u8 = 0x0D;
pub const BR_TABLE: u8 = 0x0E;
pub const RETURN: u8 = 0x0F;
pub const CALL: u8 = 0x10;
pub const CALL_INDIRECT: u8 = 0x11;
// Wasm 2.0 Tail Call extension
pub const RETURN_CALL: u8 = 0x12;
pub const RETURN_CALL_INDIRECT: u8 = 0x13;
// Exception handling opcodes (continued)
pub const DELEGATE: u8 = 0x18;
pub const CATCH_ALL: u8 = 0x19;
pub const TRY_TABLE: u8 = 0x1F;

/// WebAssembly parametric instructions
pub const DROP: u8 = 0x1A;
pub const SELECT: u8 = 0x1B;
pub const SELECT_T: u8 = 0x1C; // Typed select (for any valuetype, including V128, FuncRef, ExternRef)

/// WebAssembly variable instructions
pub const LOCAL_GET: u8 = 0x20;
pub const LOCAL_SET: u8 = 0x21;
pub const LOCAL_TEE: u8 = 0x22;
pub const GLOBAL_GET: u8 = 0x23;
pub const GLOBAL_SET: u8 = 0x24;

/// WebAssembly constant instructions
pub const I32_CONST: u8 = 0x41;
pub const I64_CONST: u8 = 0x42;
pub const F32_CONST: u8 = 0x43;
pub const F64_CONST: u8 = 0x44;

/// WebAssembly memory instructions
pub const I32_LOAD: u8 = 0x28;
pub const I64_LOAD: u8 = 0x29;
pub const F32_LOAD: u8 = 0x2A;
pub const F64_LOAD: u8 = 0x2B;
pub const I32_LOAD8_S: u8 = 0x2C;
pub const I32_LOAD8_U: u8 = 0x2D;
pub const I32_LOAD16_S: u8 = 0x2E;
pub const I32_LOAD16_U: u8 = 0x2F;
pub const I64_LOAD8_S: u8 = 0x30;
pub const I64_LOAD8_U: u8 = 0x31;
pub const I64_LOAD16_S: u8 = 0x32;
pub const I64_LOAD16_U: u8 = 0x33;
pub const I64_LOAD32_S: u8 = 0x34;
pub const I64_LOAD32_U: u8 = 0x35;
pub const I32_STORE: u8 = 0x36;
pub const I64_STORE: u8 = 0x37;
pub const F32_STORE: u8 = 0x38;
pub const F64_STORE: u8 = 0x39;
pub const I32_STORE8: u8 = 0x3A;
pub const I32_STORE16: u8 = 0x3B;
pub const I64_STORE8: u8 = 0x3C;
pub const I64_STORE16: u8 = 0x3D;
pub const I64_STORE32: u8 = 0x3E;
pub const MEMORY_SIZE: u8 = 0x3F;
pub const MEMORY_GROW: u8 = 0x40;

/// FC-prefixed opcodes (Wasm 2.0: Bulk Memory, Non-trapping Float-to-Int
/// Conversions, Table ops)
pub const PREFIX_FC: u8 = 0xFC;
// Non-trapping Float-to-Int Conversions (FC prefix)
pub const I32_TRUNC_SAT_F32_S_SUFFIX: u8 = 0x00;
pub const I32_TRUNC_SAT_F32_U_SUFFIX: u8 = 0x01;
pub const I32_TRUNC_SAT_F64_S_SUFFIX: u8 = 0x02;
pub const I32_TRUNC_SAT_F64_U_SUFFIX: u8 = 0x03;
pub const I64_TRUNC_SAT_F32_S_SUFFIX: u8 = 0x04;
pub const I64_TRUNC_SAT_F32_U_SUFFIX: u8 = 0x05;
pub const I64_TRUNC_SAT_F64_S_SUFFIX: u8 = 0x06;
pub const I64_TRUNC_SAT_F64_U_SUFFIX: u8 = 0x07;
// Bulk Memory Operations (FC prefix)
pub const MEMORY_INIT_SUFFIX: u8 = 0x08; // memory.init d:datasegidx, x:memidx (implicit 0)
pub const DATA_DROP_SUFFIX: u8 = 0x09; // data.drop d:datasegidx
pub const MEMORY_COPY_SUFFIX: u8 = 0x0A; // memory.copy d:memidx, s:memidx (implicit 0,0)
pub const MEMORY_FILL_SUFFIX: u8 = 0x0B; // memory.fill d:memidx (implicit 0)
// Table Operations (FC prefix)
pub const TABLE_INIT_SUFFIX: u8 = 0x0C; // table.init x:tableidx, e:elemsegidx
pub const ELEM_DROP_SUFFIX: u8 = 0x0D; // elem.drop e:elemsegidx
pub const TABLE_COPY_SUFFIX: u8 = 0x0E; // table.copy x:tableidx, y:tableidx
pub const TABLE_GROW_SUFFIX: u8 = 0x0F; // table.grow x:tableidx
pub const TABLE_SIZE_SUFFIX: u8 = 0x10; // table.size x:tableidx
pub const TABLE_FILL_SUFFIX: u8 = 0x11; // table.fill x:tableidx

/// Wasm 2.0 Reference Types extension opcodes
pub const REF_NULL: u8 = 0xD0; // 0xD0 ht:heaptype
pub const REF_IS_NULL: u8 = 0xD1; // 0xD1
pub const REF_FUNC: u8 = 0xD2; // 0xD2 f:funcidx

/// FD-prefixed opcodes (Wasm 2.0: Fixed-width SIMD)
pub const PREFIX_FD: u8 = 0xFD;

// SIMD Opcode Suffixes (LEB128 u32, follow PREFIX_FD)
// These are the numeric values that are LEB128 encoded after the 0xFD prefix.

// Load/Store
pub const V128_LOAD_OPCODE_SUFFIX: u32 = 0x00;
pub const V128_LOAD8X8_S_OPCODE_SUFFIX: u32 = 0x01;
pub const V128_LOAD8X8_U_OPCODE_SUFFIX: u32 = 0x02;
pub const V128_LOAD16X4_S_OPCODE_SUFFIX: u32 = 0x03;
pub const V128_LOAD16X4_U_OPCODE_SUFFIX: u32 = 0x04;
pub const V128_LOAD32X2_S_OPCODE_SUFFIX: u32 = 0x05;
pub const V128_LOAD32X2_U_OPCODE_SUFFIX: u32 = 0x06;
pub const V128_LOAD8_SPLAT_OPCODE_SUFFIX: u32 = 0x07;
pub const V128_LOAD16_SPLAT_OPCODE_SUFFIX: u32 = 0x08;
pub const V128_LOAD32_SPLAT_OPCODE_SUFFIX: u32 = 0x09;
pub const V128_LOAD64_SPLAT_OPCODE_SUFFIX: u32 = 0x0A;
pub const V128_STORE_OPCODE_SUFFIX: u32 = 0x0B;

// Constant
pub const V128_CONST_OPCODE_SUFFIX: u32 = 0x0C;

// Shuffle/Swizzle/Splat (for specific types)
pub const I8X16_SHUFFLE_OPCODE_SUFFIX: u32 = 0x0D;
pub const I8X16_SWIZZLE_OPCODE_SUFFIX: u32 = 0x0E; // Swizzle in Wasm MVP, I8x16Popcnt in Relaxed SIMD
pub const I8X16_SPLAT_OPCODE_SUFFIX: u32 = 0x0F;
pub const I16X8_SPLAT_OPCODE_SUFFIX: u32 = 0x11;
pub const I32X4_SPLAT_OPCODE_SUFFIX: u32 = 0x13;
pub const I64X2_SPLAT_OPCODE_SUFFIX: u32 = 0x15;

// i8x16 comparison
pub const I8X16_EQ_OPCODE_SUFFIX: u32 = 0x28;
pub const I8X16_NE_OPCODE_SUFFIX: u32 = 0x29;
pub const I8X16_LT_S_OPCODE_SUFFIX: u32 = 0x2A; // Wasm 2.0
pub const I8X16_LT_U_OPCODE_SUFFIX: u32 = 0x2B; // Wasm 2.0
pub const I8X16_GT_S_OPCODE_SUFFIX: u32 = 0x2C; // Wasm 2.0
pub const I8X16_GT_U_OPCODE_SUFFIX: u32 = 0x2D; // Wasm 2.0
pub const I8X16_LE_S_OPCODE_SUFFIX: u32 = 0x2E; // Wasm 2.0
pub const I8X16_LE_U_OPCODE_SUFFIX: u32 = 0x2F; // Wasm 2.0
pub const I8X16_GE_S_OPCODE_SUFFIX: u32 = 0x30; // Wasm 2.0
pub const I8X16_GE_U_OPCODE_SUFFIX: u32 = 0x31; // Wasm 2.0

// i8x16 arithmetic
pub const I8X16_ADD_OPCODE_SUFFIX: u32 = 0x38;
pub const I8X16_SUB_OPCODE_SUFFIX: u32 = 0x3A;
pub const I8X16_ABS_OPCODE_SUFFIX: u32 = 0x39; // Wasm 2.0
pub const I8X16_NEG_OPCODE_SUFFIX: u32 = 0x3B; // Wasm 2.0
pub const I8X16_ADD_SAT_S_OPCODE_SUFFIX: u32 = 0x40; // Wasm 2.0
pub const I8X16_ADD_SAT_U_OPCODE_SUFFIX: u32 = 0x41; // Wasm 2.0
pub const I8X16_SUB_SAT_S_OPCODE_SUFFIX: u32 = 0x42; // Wasm 2.0
pub const I8X16_SUB_SAT_U_OPCODE_SUFFIX: u32 = 0x43; // Wasm 2.0
pub const I8X16_SHL_OPCODE_SUFFIX: u32 = 0x49; // Wasm 2.0
pub const I8X16_SHR_S_OPCODE_SUFFIX: u32 = 0x4A; // Wasm 2.0
pub const I8X16_SHR_U_OPCODE_SUFFIX: u32 = 0x4B; // Wasm 2.0
pub const I8X16_MIN_S_OPCODE_SUFFIX: u32 = 0x4E; // Wasm 2.0
pub const I8X16_MIN_U_OPCODE_SUFFIX: u32 = 0x4F; // Wasm 2.0
pub const I8X16_MAX_S_OPCODE_SUFFIX: u32 = 0x50; // Wasm 2.0
pub const I8X16_MAX_U_OPCODE_SUFFIX: u32 = 0x51; // Wasm 2.0
// ... other i8x16 arithmetic (mul, avgr_u)

// v128 bitwise operations
pub const V128_AND_OPCODE_SUFFIX: u32 = 0x5C;
pub const V128_OR_OPCODE_SUFFIX: u32 = 0x5D;
pub const V128_XOR_OPCODE_SUFFIX: u32 = 0x5E;
pub const V128_NOT_OPCODE_SUFFIX: u32 = 0x5F;
pub const V128_ANY_TRUE_OPCODE_SUFFIX: u32 = 0x62;

// Example unary op for F32x4
pub const F32X4_ABS_OPCODE_SUFFIX: u32 = 0x9C;

// Lane Access (load/store lane)
pub const V128_LOAD8_LANE_OPCODE_SUFFIX: u32 = 0x14; // Example, there are many lane access ops
pub const V128_LOAD16_LANE_OPCODE_SUFFIX: u32 = 0x16;
pub const V128_LOAD32_LANE_OPCODE_SUFFIX: u32 = 0x18;
pub const V128_LOAD64_LANE_OPCODE_SUFFIX: u32 = 0x1A;

pub const V128_STORE8_LANE_OPCODE_SUFFIX: u32 = 0x1D;
pub const V128_STORE16_LANE_OPCODE_SUFFIX: u32 = 0x1E;
pub const V128_STORE32_LANE_OPCODE_SUFFIX: u32 = 0x1F;
pub const V128_STORE64_LANE_OPCODE_SUFFIX: u32 = 0x20;

// ... (hundreds more SIMD opcode suffixes)

/// WebAssembly numeric operation instructions
/// i32 binops
pub const I32_EQZ: u8 = 0x45;
pub const I32_EQ: u8 = 0x46;
pub const I32_NE: u8 = 0x47;
pub const I32_LT_S: u8 = 0x48;
pub const I32_LT_U: u8 = 0x49;
pub const I32_GT_S: u8 = 0x4A;
pub const I32_GT_U: u8 = 0x4B;
pub const I32_LE_S: u8 = 0x4C;
pub const I32_LE_U: u8 = 0x4D;
pub const I32_GE_S: u8 = 0x4E;
pub const I32_GE_U: u8 = 0x4F;

/// i64 binops
pub const I64_EQZ: u8 = 0x50;
pub const I64_EQ: u8 = 0x51;
pub const I64_NE: u8 = 0x52;
pub const I64_LT_S: u8 = 0x53;
pub const I64_LT_U: u8 = 0x54;
pub const I64_GT_S: u8 = 0x55;
pub const I64_GT_U: u8 = 0x56;
pub const I64_LE_S: u8 = 0x57;
pub const I64_LE_U: u8 = 0x58;
pub const I64_GE_S: u8 = 0x59;
pub const I64_GE_U: u8 = 0x5A;

/// f32 binops
pub const F32_EQ: u8 = 0x5B;
pub const F32_NE: u8 = 0x5C;
pub const F32_LT: u8 = 0x5D;
pub const F32_GT: u8 = 0x5E;
pub const F32_LE: u8 = 0x5F;
pub const F32_GE: u8 = 0x60;

/// f64 binops
pub const F64_EQ: u8 = 0x61;
pub const F64_NE: u8 = 0x62;
pub const F64_LT: u8 = 0x63;
pub const F64_GT: u8 = 0x64;
pub const F64_LE: u8 = 0x65;
pub const F64_GE: u8 = 0x66;

/// i32 unary operations
pub const I32_CLZ: u8 = 0x67;
pub const I32_CTZ: u8 = 0x68;
pub const I32_POPCNT: u8 = 0x69;

/// i32 binary operations
pub const I32_ADD: u8 = 0x6A;
pub const I32_SUB: u8 = 0x6B;
pub const I32_MUL: u8 = 0x6C;
pub const I32_DIV_S: u8 = 0x6D;
pub const I32_DIV_U: u8 = 0x6E;
pub const I32_REM_S: u8 = 0x6F;
pub const I32_REM_U: u8 = 0x70;
pub const I32_AND: u8 = 0x71;
pub const I32_OR: u8 = 0x72;
pub const I32_XOR: u8 = 0x73;
pub const I32_SHL: u8 = 0x74;
pub const I32_SHR_S: u8 = 0x75;
pub const I32_SHR_U: u8 = 0x76;
pub const I32_ROTL: u8 = 0x77;
pub const I32_ROTR: u8 = 0x78;

/// i64 unary operations
pub const I64_CLZ: u8 = 0x79;
pub const I64_CTZ: u8 = 0x7A;
pub const I64_POPCNT: u8 = 0x7B;

/// i64 binary operations
pub const I64_ADD: u8 = 0x7C;
pub const I64_SUB: u8 = 0x7D;
pub const I64_MUL: u8 = 0x7E;
pub const I64_DIV_S: u8 = 0x7F;
pub const I64_DIV_U: u8 = 0x80;
pub const I64_REM_S: u8 = 0x81;
pub const I64_REM_U: u8 = 0x82;
pub const I64_AND: u8 = 0x83;
pub const I64_OR: u8 = 0x84;
pub const I64_XOR: u8 = 0x85;
pub const I64_SHL: u8 = 0x86;
pub const I64_SHR_S: u8 = 0x87;
pub const I64_SHR_U: u8 = 0x88;
pub const I64_ROTL: u8 = 0x89;
pub const I64_ROTR: u8 = 0x8A;

/// f32 unary operations
pub const F32_ABS: u8 = 0x8B;
pub const F32_NEG: u8 = 0x8C;
pub const F32_CEIL: u8 = 0x8D;
pub const F32_FLOOR: u8 = 0x8E;
pub const F32_TRUNC: u8 = 0x8F;
pub const F32_NEAREST: u8 = 0x90;
pub const F32_SQRT: u8 = 0x91;

/// f32 binary operations
pub const F32_ADD: u8 = 0x92;
pub const F32_SUB: u8 = 0x93;
pub const F32_MUL: u8 = 0x94;
pub const F32_DIV: u8 = 0x95;
pub const F32_MIN: u8 = 0x96;
pub const F32_MAX: u8 = 0x97;
pub const F32_COPYSIGN: u8 = 0x98;

/// f64 unary operations
pub const F64_ABS: u8 = 0x99;
pub const F64_NEG: u8 = 0x9A;
pub const F64_CEIL: u8 = 0x9B;
pub const F64_FLOOR: u8 = 0x9C;
pub const F64_TRUNC: u8 = 0x9D;
pub const F64_NEAREST: u8 = 0x9E;
pub const F64_SQRT: u8 = 0x9F;

/// f64 binary operations
pub const F64_ADD: u8 = 0xA0;
pub const F64_SUB: u8 = 0xA1;
pub const F64_MUL: u8 = 0xA2;
pub const F64_DIV: u8 = 0xA3;
pub const F64_MIN: u8 = 0xA4;
pub const F64_MAX: u8 = 0xA5;
pub const F64_COPYSIGN: u8 = 0xA6;

/// Conversion operations
pub const I32_WRAP_I64: u8 = 0xA7;
pub const I32_TRUNC_F32_S: u8 = 0xA8;
pub const I32_TRUNC_F32_U: u8 = 0xA9;
pub const I32_TRUNC_F64_S: u8 = 0xAA;
pub const I32_TRUNC_F64_U: u8 = 0xAB;
pub const I64_EXTEND_I32_S: u8 = 0xAC;
pub const I64_EXTEND_I32_U: u8 = 0xAD;
pub const I64_TRUNC_F32_S: u8 = 0xAE;
pub const I64_TRUNC_F32_U: u8 = 0xAF;
pub const I64_TRUNC_F64_S: u8 = 0xB0;
pub const I64_TRUNC_F64_U: u8 = 0xB1;
pub const F32_CONVERT_I32_S: u8 = 0xB2;
pub const F32_CONVERT_I32_U: u8 = 0xB3;
pub const F32_CONVERT_I64_S: u8 = 0xB4;
pub const F32_CONVERT_I64_U: u8 = 0xB5;
pub const F32_DEMOTE_F64: u8 = 0xB6;
pub const F64_CONVERT_I32_S: u8 = 0xB7;
pub const F64_CONVERT_I32_U: u8 = 0xB8;
pub const F64_CONVERT_I64_S: u8 = 0xB9;
pub const F64_CONVERT_I64_U: u8 = 0xBA;
pub const F64_PROMOTE_F32: u8 = 0xBB;
pub const I32_REINTERPRET_F32: u8 = 0xBC;
pub const I64_REINTERPRET_F64: u8 = 0xBD;
pub const F32_REINTERPRET_I32: u8 = 0xBE;
pub const F64_REINTERPRET_I64: u8 = 0xBF;

/// Supported WebAssembly version - 1.0
pub const WASM_SUPPORTED_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

//==========================================================================
// WebAssembly Component Model Binary Format
//==========================================================================

/// Component Model magic bytes (same as core: \0asm)
pub const COMPONENT_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// Component Model binary format version - 2 bytes version, 2 bytes layer
/// Version 1.0, Layer 1
pub const COMPONENT_VERSION: [u8; 4] = [0x01, 0x00, 0x01, 0x00];

/// Component Model version only (first two bytes of version)
pub const COMPONENT_VERSION_ONLY: [u8; 2] = [0x01, 0x00];

/// Component Model layer identifier - distinguishes components from modules
pub const COMPONENT_LAYER: [u8; 2] = [0x01, 0x00];

/// Component Model section IDs
pub const COMPONENT_CUSTOM_SECTION_ID: u8 = 0x00;
pub const COMPONENT_CORE_MODULE_SECTION_ID: u8 = 0x01;
pub const COMPONENT_CORE_INSTANCE_SECTION_ID: u8 = 0x02;
pub const COMPONENT_CORE_TYPE_SECTION_ID: u8 = 0x03;
pub const COMPONENT_COMPONENT_SECTION_ID: u8 = 0x04;
pub const COMPONENT_INSTANCE_SECTION_ID: u8 = 0x05;
pub const COMPONENT_ALIAS_SECTION_ID: u8 = 0x06;
pub const COMPONENT_TYPE_SECTION_ID: u8 = 0x07;
pub const COMPONENT_CANON_SECTION_ID: u8 = 0x08;
pub const COMPONENT_START_SECTION_ID: u8 = 0x09;
pub const COMPONENT_IMPORT_SECTION_ID: u8 = 0x0A;
pub const COMPONENT_EXPORT_SECTION_ID: u8 = 0x0B;
pub const COMPONENT_VALUE_SECTION_ID: u8 = 0x0C;

/// Component Model sort kinds
pub const COMPONENT_CORE_SORT_FUNC: u8 = 0x00;
pub const COMPONENT_CORE_SORT_TABLE: u8 = 0x01;
pub const COMPONENT_CORE_SORT_MEMORY: u8 = 0x02;
pub const COMPONENT_CORE_SORT_GLOBAL: u8 = 0x03;
pub const COMPONENT_CORE_SORT_TYPE: u8 = 0x10;
pub const COMPONENT_CORE_SORT_MODULE: u8 = 0x11;
pub const COMPONENT_CORE_SORT_INSTANCE: u8 = 0x12;

pub const COMPONENT_SORT_CORE: u8 = 0x00;
pub const COMPONENT_SORT_FUNC: u8 = 0x01;
pub const COMPONENT_SORT_VALUE: u8 = 0x02;
pub const COMPONENT_SORT_TYPE: u8 = 0x03;
pub const COMPONENT_SORT_COMPONENT: u8 = 0x04;
pub const COMPONENT_SORT_INSTANCE: u8 = 0x05;
pub const COMPONENT_SORT_MODULE: u8 = 0x06;

/// Component Model value type codes
pub const COMPONENT_VALTYPE_BOOL: u8 = 0x7F;
pub const COMPONENT_VALTYPE_S8: u8 = 0x7E;
pub const COMPONENT_VALTYPE_U8: u8 = 0x7D;
pub const COMPONENT_VALTYPE_S16: u8 = 0x7C;
pub const COMPONENT_VALTYPE_U16: u8 = 0x7B;
pub const COMPONENT_VALTYPE_S32: u8 = 0x7A;
pub const COMPONENT_VALTYPE_U32: u8 = 0x79;
pub const COMPONENT_VALTYPE_S64: u8 = 0x78;
pub const COMPONENT_VALTYPE_U64: u8 = 0x77;
pub const COMPONENT_VALTYPE_F32: u8 = 0x76;
pub const COMPONENT_VALTYPE_F64: u8 = 0x75;
pub const COMPONENT_VALTYPE_CHAR: u8 = 0x74;
pub const COMPONENT_VALTYPE_STRING: u8 = 0x73;
pub const COMPONENT_VALTYPE_REF: u8 = 0x72;
pub const COMPONENT_VALTYPE_RECORD: u8 = 0x71;
pub const COMPONENT_VALTYPE_VARIANT: u8 = 0x70;
pub const COMPONENT_VALTYPE_LIST: u8 = 0x6F;
pub const COMPONENT_VALTYPE_FIXED_LIST: u8 = 0x6E;
pub const COMPONENT_VALTYPE_TUPLE: u8 = 0x6D;
pub const COMPONENT_VALTYPE_FLAGS: u8 = 0x6C;
pub const COMPONENT_VALTYPE_ENUM: u8 = 0x6B;
pub const COMPONENT_VALTYPE_OPTION: u8 = 0x6A;
pub const COMPONENT_VALTYPE_RESULT: u8 = 0x69;
pub const COMPONENT_VALTYPE_RESULT_ERR: u8 = 0x68;
pub const COMPONENT_VALTYPE_RESULT_BOTH: u8 = 0x67;
pub const COMPONENT_VALTYPE_OWN: u8 = 0x66;
pub const COMPONENT_VALTYPE_BORROW: u8 = 0x65;
pub const COMPONENT_VALTYPE_ERROR_CONTEXT: u8 = 0x64;

/// Component Model instance expression tags
pub const CORE_INSTANCE_INSTANTIATE_TAG: u8 = 0x00;
pub const CORE_INSTANCE_INLINE_EXPORTS_TAG: u8 = 0x01;

/// Component Model extern type tags
pub const EXTERN_TYPE_FUNCTION_TAG: u8 = 0x00;
pub const EXTERN_TYPE_VALUE_TAG: u8 = 0x01;
pub const EXTERN_TYPE_TYPE_TAG: u8 = 0x02;
pub const EXTERN_TYPE_INSTANCE_TAG: u8 = 0x03;
pub const EXTERN_TYPE_COMPONENT_TAG: u8 = 0x04;

/// Component Model value type tags
pub const VAL_TYPE_BOOL_TAG: u8 = 0x7F;
pub const VAL_TYPE_S8_TAG: u8 = 0x7E;
pub const VAL_TYPE_U8_TAG: u8 = 0x7D;
pub const VAL_TYPE_S16_TAG: u8 = 0x7C;
pub const VAL_TYPE_U16_TAG: u8 = 0x7B;
pub const VAL_TYPE_S32_TAG: u8 = 0x7A;
pub const VAL_TYPE_U32_TAG: u8 = 0x79;
pub const VAL_TYPE_S64_TAG: u8 = 0x78;
pub const VAL_TYPE_U64_TAG: u8 = 0x77;
pub const VAL_TYPE_F32_TAG: u8 = 0x76;
pub const VAL_TYPE_F64_TAG: u8 = 0x75;
pub const VAL_TYPE_CHAR_TAG: u8 = 0x74;
pub const VAL_TYPE_STRING_TAG: u8 = 0x73;
pub const VAL_TYPE_REF_TAG: u8 = 0x72;
pub const VAL_TYPE_RECORD_TAG: u8 = 0x71;
pub const VAL_TYPE_VARIANT_TAG: u8 = 0x70;
pub const VAL_TYPE_LIST_TAG: u8 = 0x6F;
pub const VAL_TYPE_FIXED_LIST_TAG: u8 = 0x6E;
pub const VAL_TYPE_TUPLE_TAG: u8 = 0x6D;
pub const VAL_TYPE_FLAGS_TAG: u8 = 0x6C;
pub const VAL_TYPE_ENUM_TAG: u8 = 0x6B;
pub const VAL_TYPE_OPTION_TAG: u8 = 0x6A;
pub const VAL_TYPE_RESULT_TAG: u8 = 0x69;
pub const VAL_TYPE_RESULT_ERR_TAG: u8 = 0x68;
pub const VAL_TYPE_RESULT_BOTH_TAG: u8 = 0x67;
pub const VAL_TYPE_OWN_TAG: u8 = 0x66;
pub const VAL_TYPE_BORROW_TAG: u8 = 0x65;
pub const VAL_TYPE_ERROR_CONTEXT_TAG: u8 = 0x64;

/// Parse a WebAssembly binary into a module
///
/// This is a placeholder that will be implemented fully in Phase 1.
#[cfg(feature = "std")]
pub fn parse_binary(bytes: &[u8]) -> Result<Module> {
    // Verify magic bytes
    if bytes.len() < 8 {
        return Err(parse_error("WebAssembly binary too short"));
    }

    if bytes[0..4] != WASM_MAGIC {
        return Err(parse_error("Invalid WebAssembly magic bytes"));
    }

    if bytes[4..8] != WASM_VERSION {
        return Err(parse_error("Unsupported WebAssembly version"));
    }

    // Create an empty module with the binary stored
    let mut module = Module::new();
    module.binary = Some(bytes.to_vec());

    // For now, we don't actually parse the module
    // This will be implemented in Phase 1

    Ok(module)
}

/// Binary std/no_std choice
/// needed)
pub fn read_leb128_u32(bytes: &[u8], pos: usize) -> wrt_error::Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut offset = 0;

    loop {
        if pos + offset >= bytes.len() {
            return Err(parse_error("LEB128 exceeds buffer bounds"));
        }

        let byte = bytes[pos + offset];
        offset += 1;

        // For the 5th byte (shift == 28), check for overflow:
        // - Only the low 4 bits are valid (bits 28-31 of the result)
        // - The continuation bit must not be set (would indicate > 5 bytes)
        if shift == 28 {
            // Check for continuation bit on 5th byte - too many bytes
            if (byte & 0x80) != 0 {
                return Err(parse_error("integer representation too long"));
            }
            // Check if unused high bits are set (only low 4 bits valid for u32)
            // For unsigned LEB128, bits 4-6 (0x70) must be zero
            if (byte & 0xF0) != 0 {
                return Err(parse_error("integer too large"));
            }
        }

        result |= ((byte & 0x7F) as u32) << shift;

        // Check for continuation bit
        if (byte & 0x80) == 0 {
            break;
        }

        shift += 7;

        // Prevent overflow (should not reach here if 5th byte check works)
        if shift >= 35 {
            return Err(parse_error("integer representation too long"));
        }
    }

    Ok((result, offset))
}

/// Read a LEB128 signed integer from a byte array
pub fn read_leb128_i32(bytes: &[u8], pos: usize) -> wrt_error::Result<(i32, usize)> {
    let mut result = 0i32;
    let mut shift = 0;
    let mut offset = 0;
    let mut byte;

    loop {
        if pos + offset >= bytes.len() {
            return Err(parse_error("Truncated LEB128 integer"));
        }

        byte = bytes[pos + offset];
        offset += 1;

        // For the 5th byte (shift == 28), check for overflow
        if shift == 28 {
            // Check for continuation bit on 5th byte - too many bytes
            if (byte & 0x80) != 0 {
                return Err(parse_error("integer representation too long"));
            }
            // For signed LEB128, validate the unused bits match the sign bit
            // The sign bit is bit 3 (0x08) of the 5th byte (bit 31 of result)
            // Bits 4-6 (0x70) must match the sign bit for proper sign extension
            let sign_bit = (byte & 0x08) != 0;
            let upper_bits = byte & 0x70;
            if sign_bit {
                // Negative: upper bits should all be 1 (0x70)
                if upper_bits != 0x70 {
                    return Err(parse_error("integer too large"));
                }
            } else {
                // Positive: upper bits should all be 0
                if upper_bits != 0x00 {
                    return Err(parse_error("integer too large"));
                }
            }
        }

        // Apply 7 bits from this byte
        result |= ((byte & 0x7F) as i32) << shift;
        shift += 7;

        // Check for continuation bit
        if byte & 0x80 == 0 {
            break;
        }

        // Guard against malformed/malicious LEB128 (should not reach here after 5th byte check)
        if shift >= 35 {
            return Err(parse_error("integer representation too long"));
        }
    }

    // Sign-extend if needed
    if shift < 32 && (byte & 0x40) != 0 {
        result |= !0 << shift;
    }

    Ok((result, offset))
}

/// Read a LEB128 signed 64-bit integer from a byte array
pub fn read_leb128_i64(bytes: &[u8], pos: usize) -> wrt_error::Result<(i64, usize)> {
    let mut result = 0i64;
    let mut shift = 0;
    let mut offset = 0;
    let mut byte;

    loop {
        if pos + offset >= bytes.len() {
            return Err(parse_error("Truncated LEB128 integer"));
        }

        byte = bytes[pos + offset];
        offset += 1;

        // For the 10th byte (shift == 63), check for overflow
        if shift == 63 {
            // Check for continuation bit on 10th byte - too many bytes
            if (byte & 0x80) != 0 {
                return Err(parse_error("integer representation too long"));
            }
            // For signed i64, the 10th byte contributes only 1 bit (bit 63)
            // The sign bit is bit 0 (0x01) of the 10th byte
            // Bits 1-6 (0x7E) must match the sign bit for proper sign extension
            let sign_bit = (byte & 0x01) != 0;
            let upper_bits = byte & 0x7E;
            if sign_bit {
                // Negative: upper bits should all be 1 (0x7E)
                if upper_bits != 0x7E {
                    return Err(parse_error("integer too large"));
                }
            } else {
                // Positive: upper bits should all be 0
                if upper_bits != 0x00 {
                    return Err(parse_error("integer too large"));
                }
            }
        }

        // Apply 7 bits from this byte
        result |= ((byte & 0x7F) as i64) << shift;
        shift += 7;

        // Check for continuation bit
        if byte & 0x80 == 0 {
            break;
        }

        // Guard against malformed/malicious LEB128 (should not reach here after 10th byte check)
        if shift >= 70 {
            return Err(parse_error("integer representation too long"));
        }
    }

    // Sign-extend if needed
    if shift < 64 && (byte & 0x40) != 0 {
        result |= !0 << shift;
    }

    Ok((result, offset))
}

/// Read a LEB128 unsigned 64-bit integer from a byte array
pub fn read_leb128_u64(bytes: &[u8], pos: usize) -> wrt_error::Result<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0;
    let mut offset = 0;

    loop {
        if pos + offset >= bytes.len() {
            return Err(parse_error("Truncated LEB128 integer"));
        }

        let byte = bytes[pos + offset];
        offset += 1;

        // For the 10th byte (shift == 63), check for overflow
        if shift == 63 {
            // Check for continuation bit on 10th byte - too many bytes
            if (byte & 0x80) != 0 {
                return Err(parse_error("integer representation too long"));
            }
            // For unsigned u64, the 10th byte contributes only 1 bit (bit 63)
            // Only bit 0 (0x01) is valid, bits 1-6 (0x7E) must be zero
            if (byte & 0x7E) != 0 {
                return Err(parse_error("integer too large"));
            }
        }

        // Apply 7 bits from this byte
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;

        // Check for continuation bit
        if byte & 0x80 == 0 {
            break;
        }

        // Guard against malformed/malicious LEB128 (should not reach here after 10th byte check)
        if shift >= 70 {
            return Err(parse_error("integer representation too long"));
        }
    }

    Ok((result, offset))
}

/// Read a single byte from the byte array
pub fn read_u8(bytes: &[u8], pos: usize) -> wrt_error::Result<(u8, usize)> {
    if pos >= bytes.len() {
        return Err(parse_error("Unexpected end of input"));
    }
    Ok((bytes[pos], pos + 1))
}

/// Binary std/no_std choice
pub fn read_string(bytes: &[u8], pos: usize) -> wrt_error::Result<(&[u8], usize)> {
    if pos >= bytes.len() {
        return Err(parse_error("String exceeds buffer bounds"));
    }

    // Read the string length
    let (length, length_size) = read_leb128_u32(bytes, pos)?;
    let string_start = pos + length_size;
    let string_end = string_start + length as usize;

    if string_end > bytes.len() {
        return Err(parse_error("String data exceeds buffer bounds"));
    }

    Ok((
        &bytes[string_start..string_end],
        length_size + length as usize,
    ))
}

// Binary std/no_std choice
#[cfg(feature = "std")]
pub mod with_alloc {
    use super::*;

    /// Generate a WebAssembly binary from a module
    ///
    /// This is a placeholder that will be implemented fully in Phase 1.
    #[cfg(feature = "std")]
    pub fn generate_binary(module: &Module) -> Result<Vec<u8>> {
        // If we have the original binary and haven't modified the module,
        // we can just return it
        if let Some(binary) = &module.binary {
            return Ok(binary.clone());
        }

        // Create a minimal valid module with bounded allocation
        #[cfg(feature = "safety-critical")]
        let mut binary: WrtVec<u8, { CrateId::Format as u8 }, { 4 * 1024 * 1024 }> = WrtVec::new();

        #[cfg(not(feature = "safety-critical"))]
        let mut binary = Vec::with_capacity(8);

        // Magic bytes with capacity checking
        #[cfg(feature = "safety-critical")]
        {
            for &byte in &WASM_MAGIC {
                binary.push(byte).map_err(|_| {
                    Error::runtime_execution_error(
                        "Binary generation capacity exceeded (magic bytes)",
                    )
                })?;
            }
        }
        #[cfg(not(feature = "safety-critical"))]
        binary.extend_from_slice(&WASM_MAGIC);

        // Version with capacity checking
        #[cfg(feature = "safety-critical")]
        {
            for &byte in &WASM_VERSION {
                binary.push(byte).map_err(|_| {
                    Error::runtime_execution_error("Binary generation capacity exceeded (version)")
                })?;
            }
        }
        #[cfg(not(feature = "safety-critical"))]
        binary.extend_from_slice(&WASM_VERSION);

        // Generate sections (placeholder)
        // This will be implemented in Phase 1

        // Convert to Vec<u8> for return
        #[cfg(feature = "safety-critical")]
        let result = binary.to_vec();
        #[cfg(not(feature = "safety-critical"))]
        let result = binary;

        Ok(result)
    }

    /// Read a LEB128 unsigned integer from a byte array
    ///
    /// This function will be used when implementing the full binary parser.
    pub fn read_leb128_u32(bytes: &[u8], pos: usize) -> Result<(u32, usize)> {
        let mut result = 0u32;
        let mut shift = 0;
        let mut offset = 0;

        loop {
            if pos + offset >= bytes.len() {
                return Err(parse_error("Truncated LEB128 integer"));
            }

            let byte = bytes[pos + offset];
            offset += 1;

            // For the 5th byte (shift == 28), check for overflow:
            // - Only the low 4 bits are valid (bits 28-31 of the result)
            // - The continuation bit must not be set (would indicate > 5 bytes)
            if shift == 28 {
                // Check for continuation bit on 5th byte - too many bytes
                if (byte & 0x80) != 0 {
                    return Err(parse_error("integer representation too long"));
                }
                // Check if unused high bits are set (only low 4 bits valid for u32)
                // For unsigned LEB128, bits 4-6 (0x70) must be zero
                if (byte & 0xF0) != 0 {
                    return Err(parse_error("integer too large"));
                }
            }

            // Apply 7 bits from this byte
            result |= ((byte & 0x7F) as u32) << shift;
            shift += 7;

            // Check for continuation bit
            if byte & 0x80 == 0 {
                break;
            }

            // Guard against malformed/malicious LEB128 (should not reach here due to 5th byte check)
            if shift >= 35 {
                return Err(parse_error("integer representation too long"));
            }
        }

        Ok((result, offset))
    }

    /// Read a LEB128 signed integer from a byte array
    ///
    /// This function will be used when implementing the full binary parser.
    pub fn read_leb128_i32(bytes: &[u8], pos: usize) -> Result<(i32, usize)> {
        let mut result = 0i32;
        let mut shift = 0;
        let mut offset = 0;
        let mut byte;

        loop {
            if pos + offset >= bytes.len() {
                return Err(parse_error("Truncated LEB128 integer"));
            }

            byte = bytes[pos + offset];
            offset += 1;

            // For the 5th byte (shift == 28), check for overflow
            if shift == 28 {
                // Check for continuation bit on 5th byte - too many bytes
                if (byte & 0x80) != 0 {
                    return Err(parse_error("integer representation too long"));
                }
                // For signed LEB128, validate the unused bits match the sign bit
                // The sign bit is bit 3 (0x08) of the 5th byte (bit 31 of result)
                // Bits 4-6 (0x70) must match the sign bit for proper sign extension
                let sign_bit = (byte & 0x08) != 0;
                let upper_bits = byte & 0x70;
                if sign_bit {
                    // Negative: upper bits should all be 1 (0x70)
                    if upper_bits != 0x70 {
                        return Err(parse_error("integer too large"));
                    }
                } else {
                    // Positive: upper bits should all be 0
                    if upper_bits != 0x00 {
                        return Err(parse_error("integer too large"));
                    }
                }
            }

            // Apply 7 bits from this byte
            result |= ((byte & 0x7F) as i32) << shift;
            shift += 7;

            // Check for continuation bit
            if byte & 0x80 == 0 {
                break;
            }

            // Guard against malformed/malicious LEB128 (should not reach here after 5th byte check)
            if shift >= 35 {
                return Err(parse_error("integer representation too long"));
            }
        }

        // Sign extend if needed
        if shift < 32 && (byte & 0x40) != 0 {
            // The result is negative, sign extend it
            result |= !0 << shift;
        }

        Ok((result, offset))
    }

    /// Read a LEB128 signed 64-bit integer from a byte array
    ///
    /// This function will be used when implementing the full binary parser.
    pub fn read_leb128_i64(bytes: &[u8], pos: usize) -> Result<(i64, usize)> {
        let mut result = 0i64;
        let mut shift = 0;
        let mut offset = 0;
        let mut byte;

        loop {
            if pos + offset >= bytes.len() {
                return Err(parse_error("Truncated LEB128 integer"));
            }

            byte = bytes[pos + offset];
            offset += 1;

            // For the 10th byte (shift == 63), check for overflow
            if shift == 63 {
                // Check for continuation bit on 10th byte - too many bytes
                if (byte & 0x80) != 0 {
                    return Err(parse_error("integer representation too long"));
                }
                // For signed i64, the 10th byte contributes only 1 bit (bit 63)
                // The sign bit is bit 0 (0x01) of the 10th byte
                // Bits 1-6 (0x7E) must match the sign bit for proper sign extension
                let sign_bit = (byte & 0x01) != 0;
                let upper_bits = byte & 0x7E;
                if sign_bit {
                    // Negative: upper bits should all be 1 (0x7E)
                    if upper_bits != 0x7E {
                        return Err(parse_error("integer too large"));
                    }
                } else {
                    // Positive: upper bits should all be 0
                    if upper_bits != 0x00 {
                        return Err(parse_error("integer too large"));
                    }
                }
            }

            // Apply 7 bits from this byte
            result |= ((byte & 0x7F) as i64) << shift;
            shift += 7;

            // Check for continuation bit
            if byte & 0x80 == 0 {
                break;
            }

            // Guard against malformed/malicious LEB128 (should not reach here after 10th byte check)
            if shift >= 70 {
                return Err(parse_error("integer representation too long"));
            }
        }

        // Sign extend if needed
        if shift < 64 && (byte & 0x40) != 0 {
            // The result is negative, sign extend it
            result |= !0 << shift;
        }

        Ok((result, offset))
    }

    /// Write a LEB128 unsigned integer to a byte array
    ///
    /// This function will be used when implementing the full binary generator.
    #[cfg(feature = "std")]
    pub fn write_leb128_u32(value: u32) -> Vec<u8> {
        if value == 0 {
            return vec![0];
        }

        let mut result = Vec::new();
        let mut value = value;

        while value != 0 {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;

            if value != 0 {
                byte |= 0x80;
            }

            result.push(byte);
        }

        result
    }

    /// Write a LEB128 signed integer to a byte array
    ///
    /// This function will be used when implementing the full binary generator.
    #[cfg(feature = "std")]
    pub fn write_leb128_i32(value: i32) -> Vec<u8> {
        let mut result = Vec::new();
        let mut value = value;
        let mut more = true;

        while more {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;

            // If the original value is negative, we need to sign extend
            let is_sign_bit_set = (byte & 0x40) != 0;
            let sign_extended_value = if value == 0 && !is_sign_bit_set {
                0
            } else if value == -1 && is_sign_bit_set {
                -1
            } else {
                value
            };

            more = sign_extended_value != 0 && sign_extended_value != -1;

            if more {
                byte |= 0x80;
            }

            result.push(byte);
        }

        result
    }

    /// Write a LEB128 signed 64-bit integer to a byte array
    ///
    /// This function will be used when implementing the full binary formatter.
    #[cfg(feature = "std")]
    pub fn write_leb128_i64(value: i64) -> Vec<u8> {
        let mut result = Vec::new();
        let mut value = value;
        let mut more = true;

        while more {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;

            // If the original value is negative, we need to sign extend
            let is_sign_bit_set = (byte & 0x40) != 0;
            let sign_extended_value = if value == 0 && !is_sign_bit_set {
                0
            } else if value == -1 && is_sign_bit_set {
                -1
            } else {
                value
            };

            more = sign_extended_value != 0 && sign_extended_value != -1;

            if more {
                byte |= 0x80;
            }

            result.push(byte);
        }

        result
    }

    /// Check if a binary has a valid WebAssembly header
    ///
    /// This function validates that the binary starts with the WASM_MAGIC and
    /// has a supported version.
    pub fn is_valid_wasm_header(bytes: &[u8]) -> bool {
        if bytes.len() < 8 {
            return false;
        }

        // Check magic bytes
        if bytes[0..4] != WASM_MAGIC {
            return false;
        }

        // Check version
        if bytes[4..8] != WASM_VERSION && bytes[4..8] != [0x0A, 0x6D, 0x73, 0x63] {
            return false;
        }

        true
    }

    /// Read a LEB128 unsigned 64-bit integer from a byte array
    pub fn read_leb128_u64(bytes: &[u8], pos: usize) -> Result<(u64, usize)> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        let mut offset = pos;
        let mut byte;

        loop {
            if offset >= bytes.len() {
                return Err(parse_error("Unexpected end of LEB128 sequence"));
            }

            byte = bytes[offset];
            offset += 1;

            // For the 10th byte (shift == 63), check for overflow
            if shift == 63 {
                // Check for continuation bit on 10th byte - too many bytes
                if (byte & 0x80) != 0 {
                    return Err(parse_error("integer representation too long"));
                }
                // For unsigned u64, the 10th byte contributes only 1 bit (bit 63)
                // Only bit 0 (0x01) is valid, bits 1-6 (0x7E) must be zero
                if (byte & 0x7E) != 0 {
                    return Err(parse_error("integer too large"));
                }
            }

            // Apply the 7 bits from the current byte
            result |= ((byte & 0x7F) as u64) << shift;

            // If the high bit is not set, we're done
            if byte & 0x80 == 0 {
                break;
            }

            // Otherwise, shift for the next 7 bits
            shift += 7;

            // Guard against malformed/malicious LEB128 (should not reach here after 10th byte check)
            if shift >= 70 {
                return Err(parse_error("integer representation too long"));
            }
        }

        Ok((result, offset - pos))
    }

    /// Write a LEB128 unsigned 64-bit integer to a byte array
    #[cfg(feature = "std")]
    pub fn write_leb128_u64(value: u64) -> Vec<u8> {
        let mut result = Vec::new();
        let mut value = value;

        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;

            // If there are more bits to write, set the high bit
            if value != 0 {
                byte |= 0x80;
            }

            result.push(byte);

            // If no more bits, we're done
            if value == 0 {
                break;
            }
        }

        result
    }

    /// IEEE 754 floating point handling
    ///
    /// Read a 32-bit IEEE 754 float from a byte array
    pub fn read_f32(bytes: &[u8], pos: usize) -> Result<(f32, usize)> {
        if pos + 4 > bytes.len() {
            return Err(parse_error("Not enough bytes to read f32"));
        }

        let mut buf = [0; 4];
        buf.copy_from_slice(&bytes[pos..pos + 4]);
        let value = f32::from_le_bytes(buf);
        Ok((value, pos + 4))
    }

    /// Read a 64-bit IEEE 754 float from a byte array
    pub fn read_f64(bytes: &[u8], pos: usize) -> Result<(f64, usize)> {
        if pos + 8 > bytes.len() {
            return Err(parse_error("Not enough bytes to read f64"));
        }

        let mut buf = [0; 8];
        buf.copy_from_slice(&bytes[pos..pos + 8]);
        let value = f64::from_le_bytes(buf);
        Ok((value, pos + 8))
    }

    /// Write a 32-bit IEEE 754 float to a byte array
    #[cfg(feature = "std")]
    pub fn write_f32(value: f32) -> Vec<u8> {
        let bytes = value.to_le_bytes();
        bytes.to_vec()
    }

    /// Write a 64-bit IEEE 754 float to a byte array
    #[cfg(feature = "std")]
    pub fn write_f64(value: f64) -> Vec<u8> {
        let bytes = value.to_le_bytes();
        bytes.to_vec()
    }

    /// UTF-8 string validation and parsing
    ///
    /// Validate that a byte slice contains valid UTF-8
    pub fn validate_utf8(bytes: &[u8]) -> Result<()> {
        match str::from_utf8(bytes) {
            Ok(_) => Ok(()),
            Err(_) => Err(parse_error("Invalid UTF-8 sequence")),
        }
    }

    /// Read a single byte from the byte array
    pub fn read_u8(bytes: &[u8], pos: usize) -> Result<(u8, usize)> {
        if pos >= bytes.len() {
            return Err(parse_error("Unexpected end of input"));
        }
        Ok((bytes[pos], pos + 1))
    }

    /// Read a string from a byte array
    ///
    /// This reads a length-prefixed string (used in WebAssembly names).
    pub fn read_string(bytes: &[u8], pos: usize) -> Result<(String, usize)> {
        if pos >= bytes.len() {
            return Err(parse_error("String exceeds buffer bounds"));
        }

        // Read the string length
        let (str_len, len_size) = read_leb128_u32(bytes, pos)?;
        let str_start = pos + len_size;
        let str_end = str_start + str_len as usize;

        // Ensure the string fits in the buffer
        if str_end > bytes.len() {
            return Err(parse_error("String exceeds buffer bounds"));
        }

        // Extract the string bytes
        let string_bytes = &bytes[str_start..str_end];

        // Convert to a Rust string
        match str::from_utf8(string_bytes) {
            Ok(s) => Ok((s.into(), len_size + str_len as usize)),
            Err(_) => Err(parse_error("Invalid UTF-8 in string")),
        }
    }

    /// Write a WebAssembly UTF-8 string (length prefixed)
    #[cfg(feature = "std")]
    pub fn write_string(value: &str) -> Vec<u8> {
        let mut result = Vec::new();

        // Write the length as LEB128
        let length = value.len() as u32;
        result.extend_from_slice(&write_leb128_u32(length));

        // Write the string bytes
        result.extend_from_slice(value.as_bytes());

        result
    }

    /// Read a vector from a byte array
    ///
    /// This is a generic function that reads a length-prefixed vector from a
    /// byte array, using the provided function to read each element.
    #[cfg(feature = "std")]
    pub fn read_vector<T, F>(bytes: &[u8], pos: usize, read_elem: F) -> Result<(Vec<T>, usize)>
    where
        F: Fn(&[u8], usize) -> Result<(T, usize)>,
    {
        // Read the vector length
        let (count, mut offset) = read_leb128_u32(bytes, pos)?;
        let mut result = Vec::with_capacity(count as usize);

        // Read each element
        for _ in 0..count {
            let (elem, elem_size) = read_elem(bytes, pos + offset)?;
            result.push(elem);
            offset += elem_size;
        }

        Ok((result, offset))
    }

    /// Write a vector to a byte array
    ///
    /// This is a generic function that writes a length-prefixed vector to a
    /// byte array, using the provided function to write each element.
    #[cfg(feature = "std")]
    pub fn write_vector<T, F>(elements: &[T], write_elem: F) -> Vec<u8>
    where
        F: Fn(&T) -> Vec<u8>,
    {
        let mut result = Vec::new();

        // Write the vector length
        result.extend_from_slice(&write_leb128_u32(elements.len() as u32));

        // Write each element
        for elem in elements {
            result.extend_from_slice(&write_elem(elem));
        }

        result
    }

    /// Read a section header from a byte array
    ///
    /// Returns a tuple containing the section ID, size, and new position after
    /// the header. The position should point to the start of the section
    /// content.
    pub fn read_section_header(bytes: &[u8], pos: usize) -> Result<(u8, u32, usize)> {
        if pos >= bytes.len() {
            return Err(parse_error("Attempted to read past end of binary"));
        }

        let id = bytes[pos];
        let (payload_len, len_size) = read_leb128_u32(bytes, pos + 1)?;
        Ok((id, payload_len, pos + 1 + len_size))
    }

    /// Write a section header to a byte array
    ///
    /// Writes the section ID and content size as a LEB128 unsigned integer.
    #[cfg(feature = "std")]
    pub fn write_section_header(id: u8, content_size: u32) -> Vec<u8> {
        let mut result = Vec::new();

        // Write section ID
        result.push(id);

        // Write section size
        result.extend_from_slice(&write_leb128_u32(content_size));

        result
    }

    /// Parse a block type from a byte array
    #[cfg(feature = "std")]
    pub fn parse_block_type(bytes: &[u8], pos: usize) -> Result<(FormatBlockType, usize)> {
        if pos >= bytes.len() {
            return Err(parse_error(
                "Unexpected end of input when reading block type",
            ));
        }

        let byte = bytes[pos];
        let block_type = match byte {
            // Empty block type
            0x40 => (FormatBlockType::Empty, 1),
            // Value type-based block type
            0x7F => (FormatBlockType::ValueType(ValueType::I32), 1),
            0x7E => (FormatBlockType::ValueType(ValueType::I64), 1),
            0x7D => (FormatBlockType::ValueType(ValueType::F32), 1),
            0x7C => (FormatBlockType::ValueType(ValueType::F64), 1),
            // Function type reference
            _ => {
                // If the byte is not a value type, it's a function type reference
                // which is encoded as a signed LEB128 value
                let (value, size) = read_leb128_i32(bytes, pos)?;

                // Type references are negative
                if value >= 0 {
                    return Err(parse_error(
                        "Invalid block type index: expected negative value",
                    ));
                }

                // Convert to function type index (positive)
                let func_type_idx = (-value - 1) as u32;
                (FormatBlockType::TypeIndex(func_type_idx), size)
            },
        };

        Ok(block_type)
    }

    /// Read a Component Model value type from a byte array
    #[cfg(feature = "std")]
    pub fn read_component_valtype(
        bytes: &[u8],
        pos: usize,
    ) -> Result<(crate::component::FormatValType, usize)> {
        use crate::component::FormatValType as ValType;

        if pos >= bytes.len() {
            return Err(parse_error(
                "Unexpected end of input when reading component value type",
            ));
        }

        let byte = bytes[pos];
        let mut new_pos = pos + 1;

        match byte {
            COMPONENT_VALTYPE_BOOL => Ok((ValType::Bool, new_pos)),
            COMPONENT_VALTYPE_S8 => Ok((ValType::S8, new_pos)),
            COMPONENT_VALTYPE_U8 => Ok((ValType::U8, new_pos)),
            COMPONENT_VALTYPE_S16 => Ok((ValType::S16, new_pos)),
            COMPONENT_VALTYPE_U16 => Ok((ValType::U16, new_pos)),
            COMPONENT_VALTYPE_S32 => Ok((ValType::S32, new_pos)),
            COMPONENT_VALTYPE_U32 => Ok((ValType::U32, new_pos)),
            COMPONENT_VALTYPE_S64 => Ok((ValType::S64, new_pos)),
            COMPONENT_VALTYPE_U64 => Ok((ValType::U64, new_pos)),
            COMPONENT_VALTYPE_F32 => Ok((ValType::F32, new_pos)),
            COMPONENT_VALTYPE_F64 => Ok((ValType::F64, new_pos)),
            COMPONENT_VALTYPE_CHAR => Ok((ValType::Char, new_pos)),
            COMPONENT_VALTYPE_STRING => Ok((ValType::String, new_pos)),
            COMPONENT_VALTYPE_REF => {
                // TODO: ValType::Ref variant not yet implemented
                Err(parse_error("COMPONENT_VALTYPE_REF not supported yet"))
            },
            COMPONENT_VALTYPE_RECORD => {
                let (count, next_pos) = read_leb128_u32(bytes, new_pos)?;
                new_pos = next_pos;

                // Skip the fields for now
                for _ in 0..count {
                    let (_, next_pos) = read_string(bytes, new_pos)?;
                    new_pos = next_pos;

                    let (_, next_pos) = read_component_valtype(bytes, new_pos)?;
                    new_pos = next_pos;
                }

                // Return a placeholder - proper implementation needs type store
                Err(parse_error("Record type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_VARIANT => {
                let (count, next_pos) = read_leb128_u32(bytes, new_pos)?;
                new_pos = next_pos;

                // Skip the cases for now
                for _ in 0..count {
                    let (_, next_pos) = read_string(bytes, new_pos)?;
                    new_pos = next_pos;

                    let (has_type, next_pos) = read_leb128_u32(bytes, new_pos)?;
                    new_pos = next_pos;

                    if has_type == 1 {
                        let (_, next_pos) = read_component_valtype(bytes, new_pos)?;
                        new_pos = next_pos;
                    }
                }

                // Return a placeholder - proper implementation needs type store
                Err(parse_error("Variant type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_LIST => {
                let (_, _next_pos) = read_component_valtype(bytes, new_pos)?;
                // List now uses ValTypeRef, not Box<ValType>
                // Return a placeholder - proper implementation needs type store
                Err(parse_error("List type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_FIXED_LIST => {
                let (_, next_pos) = read_component_valtype(bytes, new_pos)?;
                new_pos = next_pos;

                let (_, _next_pos) = read_leb128_u32(bytes, new_pos)?;
                // FixedList now uses ValTypeRef, not Box<ValType>
                // Return a placeholder - proper implementation needs type store
                Err(parse_error("FixedList type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_TUPLE => {
                let (count, next_pos) = read_leb128_u32(bytes, new_pos)?;
                new_pos = next_pos;

                // Skip the elements for now
                for _ in 0..count {
                    let (_, next_pos) = read_component_valtype(bytes, new_pos)?;
                    new_pos = next_pos;
                }

                // Tuple now uses BoundedVec<ValTypeRef>
                // Return a placeholder - proper implementation needs type store
                Err(parse_error("Tuple type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_FLAGS => {
                let (count, next_pos) = read_leb128_u32(bytes, new_pos)?;
                new_pos = next_pos;

                // Skip the names for now
                for _ in 0..count {
                    let (_, next_pos) = read_string(bytes, new_pos)?;
                    new_pos = next_pos;
                }

                // Flags now uses BoundedVec<WasmName>
                // Return a placeholder - proper implementation needs type store
                Err(parse_error("Flags type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_ENUM => {
                let (count, next_pos) = read_leb128_u32(bytes, new_pos)?;
                new_pos = next_pos;

                // Skip the names for now
                for _ in 0..count {
                    let (_, next_pos) = read_string(bytes, new_pos)?;
                    new_pos = next_pos;
                }

                // Enum now uses BoundedVec<WasmName>
                // Return a placeholder - proper implementation needs type store
                Err(parse_error("Enum type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_OPTION => {
                let (_, _next_pos) = read_component_valtype(bytes, new_pos)?;
                // Option now uses ValTypeRef
                // Return a placeholder - proper implementation needs type store
                Err(parse_error("Option type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_RESULT => {
                let (_, _next_pos) = read_component_valtype(bytes, new_pos)?;
                // Result now uses Option<ValTypeRef>, not Box<ValType>
                // Return a placeholder - proper implementation needs type store
                Err(parse_error("Result type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_RESULT_ERR => {
                // Convert to regular Result for backward compatibility
                let (_, _next_pos) = read_component_valtype(bytes, new_pos)?;
                // Result now uses Option<ValTypeRef>, not Box<ValType>
                // Return a placeholder - proper implementation needs type store
                Err(parse_error("Result (err) type parsing not yet implemented"))
            },
            COMPONENT_VALTYPE_RESULT_BOTH => {
                // Convert to regular Result for backward compatibility
                let (_, next_pos) = read_component_valtype(bytes, new_pos)?;
                new_pos = next_pos;

                // Read the error type
                let (_, _next_pos) = read_component_valtype(bytes, new_pos)?;
                // Result now uses Option<ValTypeRef>, not Box<ValType>
                // Return a placeholder - proper implementation needs type store
                Err(parse_error(
                    "Result (both) type parsing not yet implemented",
                ))
            },
            COMPONENT_VALTYPE_OWN => {
                let (idx, next_pos) = read_leb128_u32(bytes, new_pos)?;
                Ok((ValType::Own(idx), next_pos))
            },
            COMPONENT_VALTYPE_BORROW => {
                let (idx, next_pos) = read_leb128_u32(bytes, new_pos)?;
                Ok((ValType::Borrow(idx), next_pos))
            },
            COMPONENT_VALTYPE_ERROR_CONTEXT => Ok((ValType::ErrorContext, new_pos)),
            _ => Err(parse_error("Invalid component value type")),
        }
    }

    /// Write a Component Model value type to a byte array
    #[cfg(feature = "std")]
    pub fn write_component_valtype(val_type: &crate::component::FormatValType) -> Vec<u8> {
        use crate::component::FormatValType as ValType;
        match val_type {
            ValType::Bool => vec![COMPONENT_VALTYPE_BOOL],
            ValType::S8 => vec![COMPONENT_VALTYPE_S8],
            ValType::U8 => vec![COMPONENT_VALTYPE_U8],
            ValType::S16 => vec![COMPONENT_VALTYPE_S16],
            ValType::U16 => vec![COMPONENT_VALTYPE_U16],
            ValType::S32 => vec![COMPONENT_VALTYPE_S32],
            ValType::U32 => vec![COMPONENT_VALTYPE_U32],
            ValType::S64 => vec![COMPONENT_VALTYPE_S64],
            ValType::U64 => vec![COMPONENT_VALTYPE_U64],
            ValType::F32 => vec![COMPONENT_VALTYPE_F32],
            ValType::F64 => vec![COMPONENT_VALTYPE_F64],
            ValType::Char => vec![COMPONENT_VALTYPE_CHAR],
            ValType::String => vec![COMPONENT_VALTYPE_STRING],
            ValType::Ref(idx) => {
                let mut result = vec![COMPONENT_VALTYPE_REF];
                result.extend_from_slice(&write_leb128_u32(*idx));
                result
            },
            ValType::Record(fields) => {
                let mut result = vec![COMPONENT_VALTYPE_RECORD];
                result.extend_from_slice(&write_leb128_u32(fields.len() as u32));
                for (name, _field_type) in fields.iter() {
                    // Convert String to &str
                    result.extend_from_slice(&write_string(name));
                    // field_type is now ValTypeRef, need type store to resolve
                    result.extend_from_slice(&[0, 0, 0, 0]); // Placeholder
                }
                result
            },
            ValType::Variant(cases) => {
                let mut result = vec![COMPONENT_VALTYPE_VARIANT];
                result.extend_from_slice(&write_leb128_u32(cases.len() as u32));
                for (name, case_type) in cases.iter() {
                    // Convert String to &str
                    result.extend_from_slice(&write_string(name));
                    match case_type {
                        Some(_ty) => {
                            result.push(1); // Has type flag
                            // ty is now ValTypeRef, need type store to resolve
                            result.extend_from_slice(&[0, 0, 0, 0]); // Placeholder
                        },
                        None => {
                            result.push(0); // No type flag
                        },
                    }
                }
                result
            },
            ValType::List(_element_type) => {
                // List now uses ValTypeRef, need type store to resolve
                vec![COMPONENT_VALTYPE_LIST, 0, 0, 0, 0] // Placeholder
            },
            ValType::FixedList(_element_type, length) => {
                // FixedList now uses ValTypeRef, need type store to resolve
                let mut result = vec![COMPONENT_VALTYPE_FIXED_LIST, 0, 0, 0, 0];
                result.extend_from_slice(&write_leb128_u32(*length));
                result
            },
            ValType::Tuple(types) => {
                // Tuple now uses BoundedVec<ValTypeRef>, need type store to resolve
                let mut result = vec![COMPONENT_VALTYPE_TUPLE];
                result.extend_from_slice(&write_leb128_u32(types.len() as u32));
                // Can't write the actual types without resolving ValTypeRef
                result
            },
            ValType::Flags(names) => {
                let mut result = vec![COMPONENT_VALTYPE_FLAGS];
                result.extend_from_slice(&write_leb128_u32(names.len() as u32));
                for name in names.iter() {
                    // Convert String to &str
                    result.extend_from_slice(&write_string(name));
                }
                result
            },
            ValType::Enum(names) => {
                let mut result = vec![COMPONENT_VALTYPE_ENUM];
                result.extend_from_slice(&write_leb128_u32(names.len() as u32));
                for name in names.iter() {
                    // Convert String to &str
                    result.extend_from_slice(&write_string(name));
                }
                result
            },
            ValType::Option(_inner) => {
                // Option now uses ValTypeRef, need type store to resolve
                vec![COMPONENT_VALTYPE_OPTION, 0, 0, 0, 0] // Placeholder
            },
            ValType::Result(_) => {
                // Result now uses Option<ValTypeRef>, need type store to resolve
                vec![COMPONENT_VALTYPE_RESULT, 0, 0, 0, 0] // Placeholder
            },
            ValType::Own(type_idx) => {
                let mut result = vec![COMPONENT_VALTYPE_OWN];
                result.extend_from_slice(&write_leb128_u32(*type_idx));
                result
            },
            ValType::Borrow(type_idx) => {
                let mut result = vec![COMPONENT_VALTYPE_BORROW];
                result.extend_from_slice(&write_leb128_u32(*type_idx));
                result
            },
            ValType::Void => vec![COMPONENT_VALTYPE_ERROR_CONTEXT],
            ValType::ErrorContext => vec![COMPONENT_VALTYPE_ERROR_CONTEXT],
        }
    }

    /// Parse a WebAssembly component binary into a Component structure
    pub fn parse_component_binary(bytes: &[u8]) -> Result<crate::component::Component> {
        if bytes.len() < 8 {
            return Err(parse_error("WebAssembly component binary too short"));
        }

        // Check magic bytes
        if bytes[0..4] != COMPONENT_MAGIC {
            return Err(parse_error("Invalid WebAssembly component magic bytes"));
        }

        // Check version - be permissive about layer versions
        // Byte 4 is the layer version (0x01 for old components, 0x0A-0x0D for newer)
        // Bytes 5-7 are the component version (typically 0x01 0x00)
        let layer_version = bytes[4];
        if layer_version == 0 || layer_version > 0x1F {
            // Layer 0 is invalid, accept up to layer 31 for future compatibility
            return Err(parse_error(
                "Unsupported WebAssembly component layer version",
            ));
        }

        if bytes.len() < 10 {
            return Err(parse_error("Invalid WebAssembly component layer"));
        }

        // Create an empty component with the binary stored
        let mut component = crate::component::Component::new();
        component.binary = Some(bytes.to_vec());

        // In a real implementation, we would parse the sections here
        // This will be fully implemented in the future

        Ok(component)
    }

    /// Generate a WebAssembly component binary from a component
    pub fn generate_component_binary(component: &crate::component::Component) -> Result<Vec<u8>> {
        // If we have the original binary and haven't modified the component,
        // we can just return it
        if let Some(binary) = &component.binary {
            return Ok(binary.clone());
        }

        // Create a minimal valid component
        let mut binary = Vec::with_capacity(8);

        // Magic bytes
        binary.extend_from_slice(&COMPONENT_MAGIC);

        // Version and layer
        binary.extend_from_slice(&COMPONENT_VERSION);

        // Generate sections
        // This is a placeholder - full implementation will be added in the future

        // In a complete implementation, we would encode all sections:
        // - Core module sections
        // - Core instance sections
        // - Core type sections
        // - Component sections
        // - Instance sections
        // - Alias sections
        // - Type sections
        // - Canon sections
        // - Start sections
        // - Import sections
        // - Export sections
        // - Value sections

        Ok(binary)
    }

    /// Binary format utilities for WebAssembly
    pub struct BinaryFormat;

    impl BinaryFormat {
        /// Decode an unsigned 32-bit LEB128 integer
        pub fn decode_leb_u32(bytes: &[u8]) -> Result<(u32, usize)> {
            read_leb128_u32(bytes, 0)
        }

        /// Decode a signed 32-bit LEB128 integer
        pub fn decode_leb_i32(bytes: &[u8]) -> Result<(i32, usize)> {
            read_leb128_i32(bytes, 0)
        }

        /// Decode an unsigned 64-bit LEB128 integer
        pub fn decode_leb_u64(bytes: &[u8]) -> Result<(u64, usize)> {
            read_leb128_u64(bytes, 0)
        }

        /// Decode a signed 64-bit LEB128 integer
        pub fn decode_leb_i64(bytes: &[u8]) -> Result<(i64, usize)> {
            read_leb128_i64(bytes, 0)
        }

        /// Encode an unsigned 32-bit LEB128 integer
        pub fn encode_leb_u32(value: u32) -> Vec<u8> {
            write_leb128_u32(value)
        }

        /// Encode a signed 32-bit LEB128 integer
        pub fn encode_leb_i32(value: i32) -> Vec<u8> {
            write_leb128_i32(value)
        }

        /// Encode an unsigned 64-bit LEB128 integer
        pub fn encode_leb_u64(value: u64) -> Vec<u8> {
            write_leb128_u64(value)
        }

        /// Encode a signed 64-bit LEB128 integer
        pub fn encode_leb_i64(value: i64) -> Vec<u8> {
            write_leb128_i64(value)
        }
    }

    /// Binary std/no_std choice
    /// Returns the byte slice containing the name and the total bytes read
    /// (including length)
    pub fn read_name(bytes: &[u8], pos: usize) -> Result<(&[u8], usize)> {
        // Ensure we have enough bytes to read the string length
        if pos >= bytes.len() {
            return Err(parse_error(
                "Unexpected end of input while reading name length",
            ));
        }

        // Read the string length
        let (name_len, len_size) = read_leb128_u32(bytes, pos)?;
        let name_start = pos + len_size;

        // Ensure we have enough bytes to read the string
        if name_start + name_len as usize > bytes.len() {
            return Err(parse_error(
                "Unexpected end of input while reading name content",
            ));
        }

        // Return the slice containing the name and the total bytes read
        let name_slice = &bytes[name_start..name_start + name_len as usize];
        Ok((name_slice, pos + len_size + name_len as usize))
    }

    // STUB for parsing limits - to be fully implemented in wrt-format
    // Should parse wrt_format::types::Limits
    pub fn parse_limits(
        bytes: &[u8],
        offset: usize,
    ) -> wrt_error::Result<(crate::types::Limits, usize)> {
        if offset + 1 > bytes.len() {
            // Need at least flags byte
            return Err(Error::parse_error("Unexpected end of limits"));
        }
        let flags = bytes[offset];
        let mut current_offset = offset + 1;

        let (min, new_offset) = read_leb128_u32(bytes, current_offset)?;
        current_offset = new_offset;

        let max = if (flags & 0x01) != 0 {
            let (val, new_offset) = read_leb128_u32(bytes, current_offset)?;
            current_offset = new_offset;
            Some(val)
        } else {
            None
        };
        // Ignoring shared and memory64 flags for now as they are not in
        // wrt_format::types::Limits directly

        Ok((
            crate::types::Limits {
                min: min.into(),
                max: max.map(Into::into),
                shared: false,
                memory64: false,
            }, // Assuming default shared/memory64
            current_offset,
        ))
    }

    /// Parses an initialization expression (a sequence of instructions
    /// terminated by END). Returns the bytes of the expression (including
    /// END) and the number of bytes read.
    #[cfg(feature = "std")]
    pub fn parse_init_expr(bytes: &[u8], mut offset: usize) -> Result<(Vec<u8>, usize)> {
        let start_offset = offset;
        let mut depth = 0;

        loop {
            if offset >= bytes.len() {
                return Err(crate::error::parse_error_dynamic(format!(
                    "(offset {}): Unexpected end of data in init_expr",
                    offset
                )));
            }
            let opcode = bytes[offset];
            // We must advance offset *before* parsing immediates for that opcode.
            offset += 1;

            match opcode {
                END if depth == 0 => break, // Found the end of this expression
                BLOCK | LOOP | IF => depth += 1,
                END => {
                    // End of a nested block
                    if depth == 0 {
                        // Should have been caught by the case above
                        return Err(crate::error::parse_error_dynamic(format!(
                            "(offset {}): Mismatched END in init_expr",
                            offset - 1
                        )));
                    }
                    depth -= 1;
                },
                // Skip immediates for known const instructions
                I32_CONST => {
                    let (_, off) = read_leb128_i32(bytes, offset)?;
                    offset = off;
                },
                I64_CONST => {
                    let (_, off) = read_leb128_i64(bytes, offset)?;
                    offset = off;
                },
                F32_CONST => {
                    if offset + 4 > bytes.len() {
                        return Err(crate::error::parse_error_dynamic(format!(
                            "(offset {}): EOF in f32.const immediate",
                            offset
                        )));
                    }
                    offset += 4;
                },
                F64_CONST => {
                    if offset + 8 > bytes.len() {
                        return Err(crate::error::parse_error_dynamic(format!(
                            "(offset {}): EOF in f64.const immediate",
                            offset
                        )));
                    }
                    offset += 8;
                },
                REF_NULL => {
                    // 0xD0 ht:heap_type
                    if offset >= bytes.len() {
                        return Err(crate::error::parse_error_dynamic(format!(
                            "(offset {}): EOF in ref.null immediate",
                            offset
                        )));
                    }
                    offset += 1; // heap_type
                },
                REF_FUNC | GLOBAL_GET => {
                    // 0xD2 f:funcidx or 0x23 x:globalidx
                    let (_, off) = read_leb128_u32(bytes, offset)?;
                    offset = off;
                },
                // Other opcodes that might appear in const expressions depending on enabled
                // features. For Wasm 2.0, only the above are generally considered
                // constant. Vector ops (v128.const) could also be here if SIMD
                // consts are allowed.
                _ => { // other opcodes - assuming they have no immediates or are
                    // invalid in const expr
                },
            }
        }
        Ok((bytes[start_offset..offset].to_vec(), offset))
    }

    /// Parses an element segment from the binary format.
    /// Reference: https://webassembly.github.io/spec/core/binary/modules.html#element-section
    #[cfg(feature = "std")]
    pub fn parse_element_segment(bytes: &[u8], mut offset: usize) -> Result<(Element, usize)> {
        let (prefix_val, next_offset) = read_leb128_u32(bytes, offset).map_err(|e| {
            crate::error::parse_error_dynamic(format!(
                "Failed to read element segment prefix at offset {}: {}",
                offset, e
            ))
        })?;
        offset = next_offset;

        let (element_type, init, mode, stored_offset_expr): (
            RefType,
            ElementInit,
            PureElementMode,
            Vec<u8>,
        );

        match prefix_val {
            0x00 => {
                // MVP Active: expr vec(funcidx) end; tableidx is 0, elemkind is funcref
                let table_idx = 0;
                let (offset_expr, next_offset) = parse_init_expr(bytes, offset).map_err(|e| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Failed to parse offset_expr for element segment (type 0): {}",
                        offset, e
                    ))
                })?;
                offset = next_offset;
                let (func_indices, next_offset) = read_vector(bytes, offset, read_leb128_u32)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read func_indices for element segment (type \
                             0): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after active element segment (type 0)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                element_type = RefType::Funcref;
                init = ElementInit::FuncIndices(func_indices);
                mode = PureElementMode::Active {
                    table_index: table_idx,
                    offset_expr_len: offset_expr.len() as u32,
                };
                stored_offset_expr = offset_expr;
            },
            0x01 => {
                // Passive: elemkind vec(expr) end
                let (elemkind_byte, next_offset) = read_u8(bytes, offset)?;
                offset = next_offset;
                if elemkind_byte != 0x00 {
                    // Only funcref is supported for now
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unsupported elemkind 0x{:02X} for element segment (type 1), \
                         only funcref (0x00) supported here.",
                        offset - 1,
                        elemkind_byte
                    )));
                }
                element_type = RefType::Funcref; // funcref

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             1): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after passive element segment (type 1)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init = ElementInit::Expressions(exprs_vec);
                mode = PureElementMode::Passive;
                stored_offset_expr = Vec::new();
            },
            0x02 => {
                // Active with tableidx: tableidx expr elemkind vec(expr) end
                let (table_idx, next_offset) = read_leb128_u32(bytes, offset).map_err(|e| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Failed to read table_idx for element segment (type 2): {}",
                        offset, e
                    ))
                })?;
                offset = next_offset;
                let (offset_expr, next_offset) = parse_init_expr(bytes, offset).map_err(|e| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Failed to parse offset_expr for element segment (type 2): {}",
                        offset, e
                    ))
                })?;
                offset = next_offset;

                let (elemkind_byte, next_offset) = read_u8(bytes, offset)?;
                offset = next_offset;
                if elemkind_byte != 0x00 {
                    // Only funcref is supported for now
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unsupported elemkind 0x{:02X} for element segment (type 2), \
                         only funcref (0x00) supported here.",
                        offset - 1,
                        elemkind_byte
                    )));
                }
                element_type = RefType::Funcref; // funcref

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             2): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after active element segment (type 2)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init = ElementInit::Expressions(exprs_vec);
                mode = PureElementMode::Active {
                    table_index: table_idx,
                    offset_expr_len: offset_expr.len() as u32,
                };
                stored_offset_expr = offset_expr;
            },
            0x03 => {
                // Declared: elemkind vec(expr) end
                let (elemkind_byte, next_offset) = read_u8(bytes, offset)?;
                offset = next_offset;
                if elemkind_byte != 0x00 {
                    // Only funcref is supported for now
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unsupported elemkind 0x{:02X} for element segment (type 3), \
                         only funcref (0x00) supported here.",
                        offset - 1,
                        elemkind_byte
                    )));
                }
                element_type = RefType::Funcref; // funcref

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             3): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after declared element segment (type 3)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init = ElementInit::Expressions(exprs_vec);
                mode = PureElementMode::Declared;
                stored_offset_expr = Vec::new();
            },
            0x04 => {
                // Active with tableidx 0 (encoded in prefix): expr vec(funcidx) end
                let table_idx = 0; // Implicitly table 0 due to prefix for some interpretations, though spec shows
                // tableidx field
                let (offset_expr, next_offset) = parse_init_expr(bytes, offset).map_err(|e| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Failed to parse offset_expr for element segment (type 4): {}",
                        offset, e
                    ))
                })?;
                offset = next_offset;
                let (func_indices, next_offset) = read_vector(bytes, offset, read_leb128_u32)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read func_indices for element segment (type \
                             4): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after active element segment (type 4)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                element_type = RefType::Funcref;
                init = ElementInit::FuncIndices(func_indices);
                mode = PureElementMode::Active {
                    table_index: table_idx,
                    offset_expr_len: offset_expr.len() as u32,
                };
                stored_offset_expr = offset_expr;
            },
            0x05 => {
                // Passive: reftype vec(expr) end
                let rt_byte = bytes.get(offset).copied().ok_or_else(|| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unexpected EOF reading reftype for element segment (type 5)",
                        offset
                    ))
                })?;
                offset += 1;
                let value_type = ValueType::from_binary(rt_byte)?;
                element_type = match value_type {
                    ValueType::FuncRef => RefType::Funcref,
                    ValueType::ExternRef => RefType::Externref,
                    _ => return Err(parse_error("Invalid ref type for element")),
                };

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             5): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after passive element segment (type 5)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init = ElementInit::Expressions(exprs_vec);
                mode = PureElementMode::Passive;
                stored_offset_expr = Vec::new();
            },
            0x06 => {
                // Active with tableidx: tableidx expr reftype vec(expr) end
                let (table_idx, next_offset) = read_leb128_u32(bytes, offset).map_err(|e| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Failed to read table_idx for element segment (type 6): {}",
                        offset, e
                    ))
                })?;
                offset = next_offset;
                let (offset_expr, next_offset) = parse_init_expr(bytes, offset).map_err(|e| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Failed to parse offset_expr for element segment (type 6): {}",
                        offset, e
                    ))
                })?;
                offset = next_offset;

                let rt_byte = bytes.get(offset).copied().ok_or_else(|| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unexpected EOF reading reftype for element segment (type 6)",
                        offset
                    ))
                })?;
                offset += 1;
                let value_type = ValueType::from_binary(rt_byte)?;
                element_type = match value_type {
                    ValueType::FuncRef => RefType::Funcref,
                    ValueType::ExternRef => RefType::Externref,
                    _ => return Err(parse_error("Invalid ref type for element")),
                };

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             6): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after active element segment (type 6)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init = ElementInit::Expressions(exprs_vec);
                mode = PureElementMode::Active {
                    table_index: table_idx,
                    offset_expr_len: offset_expr.len() as u32,
                };
                stored_offset_expr = offset_expr;
            },
            0x07 => {
                // Declared: reftype vec(expr) end
                let rt_byte = bytes.get(offset).copied().ok_or_else(|| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unexpected EOF reading reftype for element segment (type 7)",
                        offset
                    ))
                })?;
                offset += 1;
                let value_type = ValueType::from_binary(rt_byte)?;
                element_type = match value_type {
                    ValueType::FuncRef => RefType::Funcref,
                    ValueType::ExternRef => RefType::Externref,
                    _ => return Err(parse_error("Invalid ref type for element")),
                };

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             7): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after declared element segment (type 7)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init = ElementInit::Expressions(exprs_vec);
                mode = PureElementMode::Declared;
                stored_offset_expr = Vec::new();
            },
            _ => {
                return Err(crate::error::parse_error_dynamic(format!(
                    "(offset {}): Invalid element segment prefix: 0x{:02X}",
                    offset.saturating_sub(1),
                    prefix_val
                )));
            },
        }

        Ok((
            Element {
                mode,
                element_type,
                init,
                offset_expr: stored_offset_expr,
            },
            offset,
        ))
    }

    /// Parses a data segment from the binary format.
    #[cfg(feature = "std")]
    pub fn parse_data(bytes: &[u8], mut offset: usize) -> Result<(PureDataSegment, usize)> {
        if offset >= bytes.len() {
            return Err(Error::parse_error(
                "Unexpected end of bytes when parsing data segment prefix",
            ));
        }

        let prefix = bytes[offset];
        offset += 1;

        match prefix {
            0x00 => {
                // Active data segment for memory 0
                let memory_index = 0; // Implicit memory index 0
                let (offset_expr, bytes_read_offset) = parse_init_expr(bytes, offset)?;
                offset += bytes_read_offset;

                let (init_byte_count, bytes_read_count) = read_leb128_u32(bytes, offset)?;
                offset += bytes_read_count;

                if offset + (init_byte_count as usize) > bytes.len() {
                    return Err(Error::parse_error(
                        "Data segment init bytes extend beyond data",
                    ));
                }
                let init_data = bytes[offset..offset + (init_byte_count as usize)].to_vec();
                offset += init_byte_count as usize;

                Ok((
                    PureDataSegment {
                        mode: PureDataMode::Active {
                            memory_index,
                            offset_expr_len: offset_expr.len() as u32,
                        },
                        offset_expr_bytes: offset_expr,
                        data_bytes: init_data,
                    },
                    offset,
                ))
            },
            0x01 => {
                // Passive data segment
                let (init_byte_count, bytes_read_count) = read_leb128_u32(bytes, offset)?;
                offset += bytes_read_count;

                if offset + (init_byte_count as usize) > bytes.len() {
                    return Err(Error::parse_error(
                        "Passive data segment init bytes extend beyond data",
                    ));
                }
                let init_data = bytes[offset..offset + (init_byte_count as usize)].to_vec();
                offset += init_byte_count as usize;

                Ok((
                    PureDataSegment {
                        mode: PureDataMode::Passive,
                        offset_expr_bytes: Vec::new(), // Not applicable for passive
                        data_bytes: init_data,
                    },
                    offset,
                ))
            },
            0x02 => {
                // Active data segment with explicit memory index
                let (memory_index, bytes_read_mem_idx) = read_leb128_u32(bytes, offset)?;
                offset += bytes_read_mem_idx;

                let (offset_expr, bytes_read_offset) = parse_init_expr(bytes, offset)?;
                offset += bytes_read_offset;

                let (init_byte_count, bytes_read_count) = read_leb128_u32(bytes, offset)?;
                offset += bytes_read_count;

                if offset + (init_byte_count as usize) > bytes.len() {
                    return Err(Error::parse_error(
                        "Data segment init bytes extend beyond data",
                    ));
                }
                let init_data = bytes[offset..offset + (init_byte_count as usize)].to_vec();
                offset += init_byte_count as usize;

                Ok((
                    PureDataSegment {
                        mode: PureDataMode::Active {
                            memory_index,
                            offset_expr_len: offset_expr.len() as u32,
                        },
                        offset_expr_bytes: offset_expr,
                        data_bytes: init_data,
                    },
                    offset,
                ))
            },
            _ => Err(crate::error::parse_error_dynamic(format!(
                "Unsupported data segment prefix: 0x{:02X}",
                prefix
            ))),
        }
    }

    /// Parses an element segment into pure format type (recommended)
    #[cfg(feature = "std")]
    pub fn parse_element_segment_pure(
        bytes: &[u8],
        mut offset: usize,
    ) -> Result<(PureElementSegment, usize)> {
        let (prefix_val, next_offset) = read_leb128_u32(bytes, offset).map_err(|e| {
            crate::error::parse_error_dynamic(format!(
                "Failed to read element segment prefix at offset {}: {}",
                offset, e
            ))
        })?;
        offset = next_offset;

        let (element_type, init_data, mode, offset_expr): (
            crate::types::RefType,
            PureElementInit,
            PureElementMode,
            Vec<u8>,
        );

        match prefix_val {
            0x00 => {
                // MVP Active: expr vec(funcidx) end; tableidx is 0, elemkind is funcref
                let table_index = 0;
                let (parsed_offset_expr, next_offset) =
                    parse_init_expr(bytes, offset).map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to parse offset_expr for element segment (type \
                             0): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;
                let (func_indices, next_offset) = read_vector(bytes, offset, read_leb128_u32)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read func_indices for element segment (type \
                             0): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after active element segment (type 0)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                element_type = crate::types::RefType::Funcref;
                init_data = PureElementInit::FunctionIndices(func_indices);
                mode = PureElementMode::Active {
                    table_index,
                    offset_expr_len: parsed_offset_expr.len() as u32,
                };
                offset_expr = parsed_offset_expr;
            },
            0x01 => {
                // Passive: elemkind vec(expr) end
                let (elemkind_byte, next_offset) = read_u8(bytes, offset)?;
                offset = next_offset;
                if elemkind_byte != 0x00 {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unsupported elemkind 0x{:02X} for element segment (type 1)",
                        offset - 1,
                        elemkind_byte
                    )));
                }
                element_type = crate::types::RefType::Funcref;

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             1): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after passive element segment (type 1)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init_data = PureElementInit::ExpressionBytes(exprs_vec);
                mode = PureElementMode::Passive;
                offset_expr = Vec::new();
            },
            0x02 => {
                // Active with tableidx: tableidx expr elemkind vec(expr) end
                let (table_index, next_offset) = read_leb128_u32(bytes, offset).map_err(|e| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Failed to read table_idx for element segment (type 2): {}",
                        offset, e
                    ))
                })?;
                offset = next_offset;
                let (parsed_offset_expr, next_offset) =
                    parse_init_expr(bytes, offset).map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to parse offset_expr for element segment (type \
                             2): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                let (elemkind_byte, next_offset) = read_u8(bytes, offset)?;
                offset = next_offset;
                if elemkind_byte != 0x00 {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unsupported elemkind 0x{:02X} for element segment (type 2)",
                        offset - 1,
                        elemkind_byte
                    )));
                }
                element_type = crate::types::RefType::Funcref;

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             2): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after active element segment (type 2)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init_data = PureElementInit::ExpressionBytes(exprs_vec);
                mode = PureElementMode::Active {
                    table_index,
                    offset_expr_len: parsed_offset_expr.len() as u32,
                };
                offset_expr = parsed_offset_expr;
            },
            0x03 => {
                // Declared: elemkind vec(expr) end
                let (elemkind_byte, next_offset) = read_u8(bytes, offset)?;
                offset = next_offset;
                if elemkind_byte != 0x00 {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unsupported elemkind 0x{:02X} for element segment (type 3)",
                        offset - 1,
                        elemkind_byte
                    )));
                }
                element_type = crate::types::RefType::Funcref;

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             3): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after declared element segment (type 3)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init_data = PureElementInit::ExpressionBytes(exprs_vec);
                mode = PureElementMode::Declared;
                offset_expr = Vec::new();
            },
            0x04 => {
                // Active with tableidx 0 (encoded in prefix): expr vec(funcidx) end
                let table_index = 0; // Implicitly table 0
                let (parsed_offset_expr, next_offset) =
                    parse_init_expr(bytes, offset).map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to parse offset_expr for element segment (type \
                             4): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;
                let (func_indices, next_offset) = read_vector(bytes, offset, read_leb128_u32)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read func_indices for element segment (type \
                             4): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after active element segment (type 4)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                element_type = crate::types::RefType::Funcref;
                init_data = PureElementInit::FunctionIndices(func_indices);
                mode = PureElementMode::Active {
                    table_index,
                    offset_expr_len: parsed_offset_expr.len() as u32,
                };
                offset_expr = parsed_offset_expr;
            },
            0x05 => {
                // Passive: reftype vec(expr) end
                let rt_byte = bytes.get(offset).copied().ok_or_else(|| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unexpected EOF reading reftype for element segment (type 5)",
                        offset
                    ))
                })?;
                offset += 1;
                let value_type = ValueType::from_binary(rt_byte)?;
                element_type = match value_type {
                    ValueType::FuncRef => crate::types::RefType::Funcref,
                    ValueType::ExternRef => crate::types::RefType::Externref,
                    _ => return Err(parse_error("Invalid ref type for element")),
                };

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             5): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after passive element segment (type 5)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init_data = PureElementInit::ExpressionBytes(exprs_vec);
                mode = PureElementMode::Passive;
                offset_expr = Vec::new();
            },
            0x06 => {
                // Active with tableidx: tableidx expr reftype vec(expr) end
                let (table_index, next_offset) = read_leb128_u32(bytes, offset).map_err(|e| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Failed to read table_idx for element segment (type 6): {}",
                        offset, e
                    ))
                })?;
                offset = next_offset;
                let (parsed_offset_expr, next_offset) =
                    parse_init_expr(bytes, offset).map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to parse offset_expr for element segment (type \
                             6): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                let rt_byte = bytes.get(offset).copied().ok_or_else(|| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unexpected EOF reading reftype for element segment (type 6)",
                        offset
                    ))
                })?;
                offset += 1;
                let value_type = ValueType::from_binary(rt_byte)?;
                element_type = match value_type {
                    ValueType::FuncRef => crate::types::RefType::Funcref,
                    ValueType::ExternRef => crate::types::RefType::Externref,
                    _ => return Err(parse_error("Invalid ref type for element")),
                };

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             6): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after active element segment (type 6)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init_data = PureElementInit::ExpressionBytes(exprs_vec);
                mode = PureElementMode::Active {
                    table_index,
                    offset_expr_len: parsed_offset_expr.len() as u32,
                };
                offset_expr = parsed_offset_expr;
            },
            0x07 => {
                // Declared: reftype vec(expr) end
                let rt_byte = bytes.get(offset).copied().ok_or_else(|| {
                    crate::error::parse_error_dynamic(format!(
                        "(offset {}): Unexpected EOF reading reftype for element segment (type 7)",
                        offset
                    ))
                })?;
                offset += 1;
                let value_type = ValueType::from_binary(rt_byte)?;
                element_type = match value_type {
                    ValueType::FuncRef => crate::types::RefType::Funcref,
                    ValueType::ExternRef => crate::types::RefType::Externref,
                    _ => return Err(parse_error("Invalid ref type for element")),
                };

                let (exprs_vec, next_offset) = read_vector(bytes, offset, parse_init_expr)
                    .map_err(|e| {
                        crate::error::parse_error_dynamic(format!(
                            "(offset {}): Failed to read expressions for element segment (type \
                             7): {}",
                            offset, e
                        ))
                    })?;
                offset = next_offset;

                if bytes.get(offset).copied() != Some(END) {
                    return Err(crate::error::parse_error_dynamic(format!(
                        "(offset {}): Expected END opcode after declared element segment (type 7)",
                        offset
                    )));
                }
                offset += 1; // Consume END

                init_data = PureElementInit::ExpressionBytes(exprs_vec);
                mode = PureElementMode::Declared;
                offset_expr = Vec::new();
            },
            _ => {
                return Err(crate::error::parse_error_dynamic(format!(
                    "(offset {}): Invalid element segment prefix: 0x{:02X}",
                    offset.saturating_sub(1),
                    prefix_val
                )));
            },
        }

        Ok((
            PureElementSegment {
                mode,
                element_type,
                offset_expr_bytes: offset_expr,
                init_data,
            },
            offset,
        ))
    }
} // Binary std/no_std choice

// No-std write functions

/// Write a LEB128 unsigned integer to a byte array (no_std version)
///
/// Returns the number of bytes written to the buffer.
/// Buffer must be at least 5 bytes long (max size for u32 LEB128).
#[cfg(not(any(feature = "std")))]
pub fn write_leb128_u32_to_slice(value: u32, buffer: &mut [u8]) -> wrt_error::Result<usize> {
    if buffer.len() < 5 {
        return Err(parse_error("Buffer too small for LEB128 encoding"));
    }

    if value == 0 {
        buffer[0] = 0;
        return Ok(1);
    }

    let mut value = value;
    let mut position = 0;

    while value != 0 {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80;
        }

        buffer[position] = byte;
        position += 1;
    }

    Ok(position)
}

/// Write a string to a byte array in WebAssembly format (no_std version)
///
/// The format is: length (LEB128) followed by UTF-8 bytes
/// Returns the number of bytes written.
#[cfg(not(any(feature = "std")))]
pub fn write_string_to_slice(value: &str, buffer: &mut [u8]) -> wrt_error::Result<usize> {
    let str_bytes = value.as_bytes();
    let length = str_bytes.len() as u32;

    // First calculate how many bytes we need for the length
    let mut length_buffer = [0u8; 5];
    let length_bytes = write_leb128_u32_to_slice(length, &mut length_buffer)?;

    // Check if buffer is large enough
    let total_size = length_bytes + str_bytes.len();
    if buffer.len() < total_size {
        return Err(parse_error("Buffer too small for string encoding"));
    }

    // Write the length
    buffer[..length_bytes].copy_from_slice(&length_buffer[..length_bytes]);

    // Write the string bytes
    buffer[length_bytes..total_size].copy_from_slice(str_bytes);

    Ok(total_size)
}

/// Write a LEB128 u32 to a BoundedVec (no_std version)
#[cfg(not(any(feature = "std")))]
pub fn write_leb128_u32_bounded<
    const N: usize,
    P: wrt_foundation::MemoryProvider + Clone + Default + Eq,
>(
    value: u32,
    vec: &mut wrt_foundation::BoundedVec<u8, N, P>,
) -> wrt_error::Result<()> {
    let mut buffer = [0u8; 5];
    let bytes_written = write_leb128_u32_to_slice(value, &mut buffer)?;

    for i in 0..bytes_written {
        vec.push(buffer[i]).map_err(|_| parse_error("BoundedVec capacity exceeded"))?;
    }

    Ok(())
}

/// Write a string to a BoundedVec (no_std version)
#[cfg(not(any(feature = "std")))]
pub fn write_string_bounded<
    const N: usize,
    P: wrt_foundation::MemoryProvider + Clone + Default + Eq,
>(
    value: &str,
    vec: &mut wrt_foundation::BoundedVec<u8, N, P>,
) -> wrt_error::Result<()> {
    // Write length
    write_leb128_u32_bounded(value.len() as u32, vec)?;

    // Write string bytes
    vec.extend_from_slice(value.as_bytes())
        .map_err(|_| parse_error("BoundedVec capacity exceeded"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Define test helper functions directly here since imports aren't working
    // Read functions
    fn read_f32_test(bytes: &[u8], pos: usize) -> crate::Result<(f32, usize)> {
        if pos + 4 > bytes.len() {
            return Err(parse_error("Not enough bytes to read f32"));
        }

        let mut buf = [0; 4];
        buf.copy_from_slice(&bytes[pos..pos + 4]);
        let value = f32::from_le_bytes(buf);
        Ok((value, pos + 4))
    }

    fn read_f64_test(bytes: &[u8], pos: usize) -> crate::Result<(f64, usize)> {
        if pos + 8 > bytes.len() {
            return Err(parse_error("Not enough bytes to read f64"));
        }

        let mut buf = [0; 8];
        buf.copy_from_slice(&bytes[pos..pos + 8]);
        let value = f64::from_le_bytes(buf);
        Ok((value, pos + 8))
    }

    #[cfg(feature = "std")]
    fn read_string_test(bytes: &[u8], pos: usize) -> crate::Result<(String, usize)> {
        if pos >= bytes.len() {
            return Err(parse_error("String exceeds buffer bounds"));
        }

        // Read the string length using parent module function
        let (str_len, len_size) = read_leb128_u32(bytes, pos)?;
        let str_start = pos + len_size;
        let str_end = str_start + str_len as usize;

        if str_end > bytes.len() {
            return Err(parse_error("String exceeds buffer bounds"));
        }

        let string_bytes = &bytes[str_start..str_end];
        match core::str::from_utf8(string_bytes) {
            Ok(s) => Ok((s.into(), len_size + str_len as usize)),
            Err(_) => Err(parse_error("Invalid UTF-8 in string")),
        }
    }

    #[cfg(feature = "std")]
    fn read_vector_test<T, F>(
        bytes: &[u8],
        pos: usize,
        read_elem: F,
    ) -> crate::Result<(Vec<T>, usize)>
    where
        F: Fn(&[u8], usize) -> crate::Result<(T, usize)>,
    {
        let (count, mut offset) = read_leb128_u32(bytes, pos)?;
        let mut result = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let (elem, elem_size) = read_elem(bytes, pos + offset)?;
            result.push(elem);
            offset += elem_size;
        }

        Ok((result, offset))
    }

    fn read_section_header_test(bytes: &[u8], pos: usize) -> crate::Result<(u8, u32, usize)> {
        if pos >= bytes.len() {
            return Err(parse_error("Attempted to read past end of binary"));
        }

        let id = bytes[pos];
        let (payload_len, len_size) = read_leb128_u32(bytes, pos + 1)?;
        Ok((id, payload_len, pos + 1 + len_size))
    }

    fn validate_utf8_test(bytes: &[u8]) -> crate::Result<()> {
        match core::str::from_utf8(bytes) {
            Ok(_) => Ok(()),
            Err(_) => Err(parse_error("Invalid UTF-8 sequence")),
        }
    }

    // Write functions
    #[cfg(feature = "std")]
    fn write_leb128_u32_test(value: u32) -> Vec<u8> {
        if value == 0 {
            return vec![0];
        }

        let mut result = Vec::new();
        let mut value = value;

        while value != 0 {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;

            if value != 0 {
                byte |= 0x80;
            }

            result.push(byte);
        }

        result
    }

    #[cfg(feature = "std")]
    fn write_f32_test(value: f32) -> Vec<u8> {
        let bytes = value.to_le_bytes();
        bytes.to_vec()
    }

    #[cfg(feature = "std")]
    fn write_f64_test(value: f64) -> Vec<u8> {
        let bytes = value.to_le_bytes();
        bytes.to_vec()
    }

    #[cfg(feature = "std")]
    fn write_string_test(value: &str) -> Vec<u8> {
        let mut result = Vec::new();
        let length = value.len() as u32;
        result.extend_from_slice(&write_leb128_u32_test(length));
        result.extend_from_slice(value.as_bytes());
        result
    }

    #[cfg(feature = "std")]
    fn write_leb128_u64_test(value: u64) -> Vec<u8> {
        let mut result = Vec::new();
        let mut value = value;

        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;

            if value != 0 {
                byte |= 0x80;
            }

            result.push(byte);

            if value == 0 {
                break;
            }
        }

        result
    }

    #[cfg(feature = "std")]
    fn write_vector_test<T, F>(elements: &[T], write_elem: F) -> Vec<u8>
    where
        F: Fn(&T) -> Vec<u8>,
    {
        let mut result = Vec::new();
        result.extend_from_slice(&write_leb128_u32_test(elements.len() as u32));
        for elem in elements {
            result.extend_from_slice(&write_elem(elem));
        }
        result
    }

    #[cfg(feature = "std")]
    fn write_section_header_test(id: u8, content_size: u32) -> Vec<u8> {
        let mut result = Vec::new();
        result.push(id);
        result.extend_from_slice(&write_leb128_u32_test(content_size));
        result
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_f32_roundtrip() {
        let values = [
            0.0f32,
            -0.0,
            1.0,
            -1.0,
            3.14159,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
        ];

        for &value in &values {
            let bytes = write_f32_test(value);
            let (decoded, size) = read_f32_test(&bytes, 0).unwrap();

            assert_eq!(size, 4);
            if value.is_nan() {
                assert!(decoded.is_nan());
            } else {
                assert_eq!(decoded, value);
            }
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_f64_roundtrip() {
        let values = [
            0.0f64,
            -0.0,
            1.0,
            -1.0,
            3.14159265358979,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];

        for &value in &values {
            let bytes = write_f64_test(value);
            let (decoded, size) = read_f64_test(&bytes, 0).unwrap();

            assert_eq!(size, 8);
            if value.is_nan() {
                assert!(decoded.is_nan());
            } else {
                assert_eq!(decoded, value);
            }
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_string_roundtrip() {
        let test_strings = [
            "",
            "Hello, World!",
            "UTF-8 test: ñáéíóú",
            "🦀 Rust is awesome!",
        ];

        for &s in &test_strings {
            let bytes = write_string_test(s);
            let (decoded, _) = read_string_test(&bytes, 0).unwrap();

            assert_eq!(decoded, s);
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_leb128_u64_roundtrip() {
        let test_values = [
            0u64,
            1,
            127,
            128,
            16_384,
            0x7FFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF_FFFF_FFFF,
        ];

        for &value in &test_values {
            let bytes = write_leb128_u64_test(value);
            let (decoded, _) = read_leb128_u64(&bytes, 0).unwrap();

            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn test_utf8_validation() {
        // Valid UTF-8
        assert!(validate_utf8_test(b"Hello").is_ok());
        assert!(validate_utf8_test("🦀 Rust".as_bytes()).is_ok());

        // Invalid UTF-8
        let invalid_utf8 = [0xFF, 0xFE, 0xFD];
        assert!(validate_utf8_test(&invalid_utf8).is_err());
    }

    #[test]
    fn test_leb128_u32_overflow_too_many_bytes() {
        // 6 bytes for u32 - "integer representation too long"
        // This is 6 bytes with continuation bits: value 2 with one byte too many
        let bytes = [0x82, 0x80, 0x80, 0x80, 0x80, 0x00];
        let result = read_leb128_u32(&bytes, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message().contains("integer representation too long"),
            "Expected 'integer representation too long', got: {}",
            err.message()
        );
    }

    #[test]
    fn test_leb128_u32_overflow_unused_bits_set() {
        // 5 bytes with unused bits set - "integer too large"
        // This is value 2 with unused bits (0x10) set in the 5th byte
        let bytes = [0x82, 0x80, 0x80, 0x80, 0x10];
        let result = read_leb128_u32(&bytes, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message().contains("integer too large"),
            "Expected 'integer too large', got: {}",
            err.message()
        );
    }

    #[test]
    fn test_leb128_u32_valid_max_bytes() {
        // Valid 5-byte encoding of 0xFFFFFFFF
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
        let result = read_leb128_u32(&bytes, 0);
        assert!(result.is_ok());
        let (value, _) = result.unwrap();
        assert_eq!(value, 0xFFFFFFFF);
    }

    #[test]
    fn test_leb128_i32_overflow_too_many_bytes() {
        // 6 bytes for i32 - "integer representation too long"
        let bytes = [0x80, 0x80, 0x80, 0x80, 0x80, 0x00];
        let result = read_leb128_i32(&bytes, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message().contains("integer representation too long"),
            "Expected 'integer representation too long', got: {}",
            err.message()
        );
    }

    #[test]
    fn test_leb128_i32_overflow_unused_bits_positive() {
        // 5 bytes with incorrect unused bits for positive value - "integer too large"
        // Value 0 with unused bits set (0x70 should be 0x00 for positive)
        let bytes = [0x80, 0x80, 0x80, 0x80, 0x70];
        let result = read_leb128_i32(&bytes, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message().contains("integer too large"),
            "Expected 'integer too large', got: {}",
            err.message()
        );
    }

    #[test]
    fn test_leb128_i32_overflow_unused_bits_negative() {
        // 5 bytes with incorrect unused bits for negative value - "integer too large"
        // Value -1 with unused bits unset (0x0F instead of 0x7F)
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
        let result = read_leb128_i32(&bytes, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message().contains("integer too large"),
            "Expected 'integer too large', got: {}",
            err.message()
        );
    }

    #[test]
    fn test_leb128_i32_valid_negative() {
        // Valid 5-byte encoding of -1
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0x7F];
        let result = read_leb128_i32(&bytes, 0);
        assert!(result.is_ok());
        let (value, _) = result.unwrap();
        assert_eq!(value, -1);
    }

    #[test]
    fn test_leb128_i32_valid_positive() {
        // Valid 5-byte encoding of max positive i32 (0x7FFFFFFF)
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0x07];
        let result = read_leb128_i32(&bytes, 0);
        assert!(result.is_ok());
        let (value, _) = result.unwrap();
        assert_eq!(value, i32::MAX);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_read_write_vector() {
        // Create a test vector of u32 values
        let values = vec![1u32, 42, 100, 1000];

        // Write the vector
        let bytes = write_vector_test(&values, |v| write_leb128_u32_test(*v));

        // Read the vector back
        let (decoded, _) = read_vector_test(&bytes, 0, read_leb128_u32).unwrap();

        assert_eq!(values, decoded);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_section_header() {
        // Create a section header for a type section with 10 bytes of content
        let section_id = TYPE_SECTION_ID;
        let content_size = 10;

        let bytes = write_section_header_test(section_id, content_size);

        // Read the section header back
        let (decoded_id, decoded_size, _) = read_section_header_test(&bytes, 0).unwrap();

        assert_eq!(section_id, decoded_id);
        assert_eq!(content_size, decoded_size);
    }

    // Re-export pure parsing functions for std builds
    // Note: parse_data and parse_element_segment_pure have been removed or renamed
    // pub use super::{
    //     parse_data,
    //     parse_element_segment_pure,
    // };
}

// Additional exports and aliases for compatibility

// Note: parse_vec functionality is handled by other parsing functions

// Helper function to read a u32 (4 bytes, little-endian) from a byte array
pub fn read_u32(bytes: &[u8], pos: usize) -> wrt_error::Result<(u32, usize)> {
    if pos + 4 > bytes.len() {
        return Err(parse_error("Truncated u32"));
    }
    let value = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
    Ok((value, 4))
}
