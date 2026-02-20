// WRT - wrt-error
// Module: WRT Error Codes
// SW-REQ-ID: REQ_004
// SW-REQ-ID: REQ_ERROR_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Error codes for WRT

/// Stack underflow error
pub const STACK_UNDERFLOW: u16 = 1000;
/// Stack overflow error
pub const STACK_OVERFLOW: u16 = 1001;
/// Unaligned memory access error
pub const UNALIGNED_MEMORY_ACCESS: u16 = 1002;
/// Invalid memory access error
pub const INVALID_MEMORY_ACCESS: u16 = 1003;
/// Invalid instance index error
pub const INVALID_INSTANCE_INDEX: u16 = 1004;
/// General execution error
pub const EXECUTION_ERROR: u16 = 1005;
/// Feature not implemented error
pub const NOT_IMPLEMENTED: u16 = 1006;
/// Memory access error
pub const MEMORY_ACCESS_ERROR: u16 = 1007;
/// Initialization error
pub const INITIALIZATION_ERROR: u16 = 1008;
/// Type mismatch error
pub const TYPE_MISMATCH: u16 = 1009;
/// Parse error
pub const PARSE_ERROR: u16 = 1010;
/// Invalid version error
pub const INVALID_VERSION: u16 = 1011;
/// Invalid operation error
pub const INVALID_OPERATION: u16 = 1019;
/// Instance not found error
pub const INSTANCE_NOT_FOUND: u16 = 1020;
/// Threading error
pub const THREADING_ERROR: u16 = 1021;
/// Cleanup failed error
pub const CLEANUP_FAILED: u16 = 1022;
/// Function call failed error
pub const FUNCTION_CALL_FAILED: u16 = 1023;
/// Type conversion error
pub const TYPE_CONVERSION_ERROR: u16 = 1024;
/// Configuration error
pub const CONFIGURATION_ERROR: u16 = 1025;
/// Operation cancelled error
pub const OPERATION_CANCELLED: u16 = 1026;
/// Out of bounds error
pub const OUT_OF_BOUNDS_ERROR: u16 = 1012;
/// Execution instruction index out of bounds error
pub const EXECUTION_INSTRUCTION_INDEX_OUT_OF_BOUNDS: u16 = 1013;
/// Execution invalid frame error
pub const EXECUTION_INVALID_FRAME: u16 = 1014;
/// Execution reader not implemented error
pub const EXECUTION_READER_NOT_IMPLEMENTED: u16 = 1015;
/// Capacity exceeded
pub const CAPACITY_EXCEEDED: u16 = 1016;
/// Gas limit exceeded
pub const GAS_LIMIT_EXCEEDED: u16 = 1017;
/// Call stack exhausted
pub const CALL_STACK_EXHAUSTED: u16 = 1018;

// Component model error codes (2000-2999)
/// Invalid function index error
pub const INVALID_FUNCTION_INDEX: u16 = 2000;
/// Component type mismatch error
pub const COMPONENT_TYPE_MISMATCH: u16 = 2001;
/// Encoding error
pub const ENCODING_ERROR: u16 = 2002;
/// Execution limit exceeded error
pub const EXECUTION_LIMIT_EXCEEDED: u16 = 2003;
/// Component instantiation error
pub const COMPONENT_INSTANTIATION_ERROR: u16 = 2004;
/// Canonical ABI error
pub const CANONICAL_ABI_ERROR: u16 = 2005;
/// Component linking error
pub const COMPONENT_LINKING_ERROR: u16 = 2006;

// Resource error codes (3000-3999)
/// Resource error
pub const RESOURCE_ERROR: u16 = 3000;
/// Resource limit exceeded error
pub const RESOURCE_LIMIT_EXCEEDED: u16 = 3001;
/// Resource access error
pub const RESOURCE_ACCESS_ERROR: u16 = 3002;
/// Resource not found error
pub const RESOURCE_NOT_FOUND: u16 = 3003;
/// Resource invalid handle error
pub const RESOURCE_INVALID_HANDLE: u16 = 3004;
/// Global not found
pub const GLOBAL_NOT_FOUND: u16 = 3005;
/// Memory not found
pub const MEMORY_NOT_FOUND: u16 = 3006;
/// Table not found
pub const TABLE_NOT_FOUND: u16 = 3007;
/// Resource exhausted error
pub const RESOURCE_EXHAUSTED: u16 = 3008;
/// WASI invalid file descriptor
pub const WASI_INVALID_FD: u16 = 3009;
/// WASI permission denied
pub const WASI_PERMISSION_DENIED: u16 = 3010;
/// WASI resource limit
pub const WASI_RESOURCE_LIMIT: u16 = 3011;
/// WASI invalid argument
pub const WASI_INVALID_ARGUMENT: u16 = 3012;
/// WASI invalid encoding
pub const WASI_INVALID_ENCODING: u16 = 3013;
/// WASI runtime error
pub const WASI_RUNTIME_ERROR: u16 = 3014;
/// WASI resource exhausted
pub const WASI_RESOURCE_EXHAUSTED: u16 = 3015;
/// WASI unsupported operation
pub const WASI_UNSUPPORTED_OPERATION: u16 = 3016;
/// WASI verification failed
pub const WASI_VERIFICATION_FAILED: u16 = 3017;
/// WASI timeout
pub const WASI_TIMEOUT: u16 = 3018;
/// Allocation failed error
pub const ALLOCATION_FAILED: u16 = 3012;
/// Memory access denied through wrapper
pub const MEMORY_ACCESS_DENIED: u16 = 3013;
/// Table access denied through wrapper
pub const TABLE_ACCESS_DENIED: u16 = 3014;
/// Global access denied through wrapper
pub const GLOBAL_ACCESS_DENIED: u16 = 3015;

// Memory error codes (4000-4999)
/// Memory out of bounds error
pub const MEMORY_OUT_OF_BOUNDS: u16 = 4000;
/// Memory grow error
pub const MEMORY_GROW_ERROR: u16 = 4001;
/// Memory access out of bounds error
pub const MEMORY_ACCESS_OUT_OF_BOUNDS: u16 = 4002;
/// Memory access unaligned error
pub const MEMORY_ACCESS_UNALIGNED: u16 = 4003;

// Validation error codes (5000-5999)
/// Validation error
pub const VALIDATION_ERROR: u16 = 5000;
/// Validation failure
pub const VALIDATION_FAILURE: u16 = 5001;
/// Invalid argument error
pub const INVALID_ARGUMENT: u16 = 5002;
/// Invalid state error
pub const INVALID_STATE: u16 = 5003;
/// Checksum mismatch error
pub const CHECKSUM_MISMATCH: u16 = 5004;
/// Integrity violation error
pub const INTEGRITY_VIOLATION: u16 = 5005;
/// Verification level violation error
pub const VERIFICATION_LEVEL_VIOLATION: u16 = 5006;
/// Validation global type mismatch error
pub const VALIDATION_GLOBAL_TYPE_MISMATCH: u16 = 5007;
/// Validation invalid memory index error
pub const VALIDATION_INVALID_MEMORY_INDEX: u16 = 5027;
/// Validation invalid global index error
pub const VALIDATION_INVALID_GLOBAL_INDEX: u16 = 5028;
/// Validation unsupported feature error
pub const VALIDATION_UNSUPPORTED_FEATURE: u16 = 5008;
/// Validation invalid instruction error
pub const VALIDATION_INVALID_INSTRUCTION: u16 = 5009;
/// Validation empty stack error
pub const VALIDATION_EMPTY_STACK: u16 = 5010;
/// Validation stack size error
pub const VALIDATION_STACK_SIZE_ERROR: u16 = 5011;
/// Validation no binary error
pub const VALIDATION_NO_BINARY: u16 = 5012;
/// Validation function not found error
pub const VALIDATION_FUNCTION_NOT_FOUND: u16 = 5013;
/// Validation export not found error
pub const VALIDATION_EXPORT_NOT_FOUND: u16 = 5014;
/// Validation invalid function type error
pub const VALIDATION_INVALID_FUNCTION_TYPE: u16 = 5015;
/// Validation invalid table index error
pub const VALIDATION_INVALID_TABLE_INDEX: u16 = 5016;
/// Validation invalid element index error
pub const VALIDATION_INVALID_ELEMENT_INDEX: u16 = 5017;
/// Validation invalid data segment index error
pub const VALIDATION_INVALID_DATA_SEGMENT_INDEX: u16 = 5018;
/// Validation duplicate table reference error
pub const VALIDATION_DUPLICATE_TABLE_REFERENCE: u16 = 5019;
/// Validation invalid frame index error
pub const VALIDATION_INVALID_FRAME_INDEX: u16 = 5020;
/// Validation stack underflow error
pub const VALIDATION_STACK_UNDERFLOW: u16 = 5021;
/// Validation: min limit from u64 source exceeds u32 target
pub const VALIDATION_LIMIT_MIN_EXCEEDS_U32: u16 = 5022;
/// Validation: max limit from u64 source exceeds u32 target
pub const VALIDATION_LIMIT_MAX_EXCEEDS_U32: u16 = 5023;
/// Validation: max limit is less than min limit
pub const VALIDATION_LIMIT_MAX_LESS_THAN_MIN: u16 = 5024;
/// Validation: Invalid custom section name
pub const VALIDATION_INVALID_CUSTOM_SECTION_NAME: u16 = 5025;
/// Validation: Custom section data too long
pub const VALIDATION_CUSTOM_SECTION_DATA_TOO_LONG: u16 = 5026;

// Type error codes (6000-6999)
/// Invalid type error
pub const INVALID_TYPE: u16 = 6000;
/// Type mismatch error
pub const TYPE_MISMATCH_ERROR: u16 = 6001;
/// Invalid function type error
pub const INVALID_FUNCTION_TYPE: u16 = 6002;
/// Invalid value type error
pub const INVALID_VALUE_TYPE: u16 = 6003;
/// Parse invalid function index type error
pub const PARSE_INVALID_FUNCTION_INDEX_TYPE: u16 = 6004;
/// Parse invalid table index type error
pub const PARSE_INVALID_TABLE_INDEX_TYPE: u16 = 6005;
/// Parse invalid memory index type error
pub const PARSE_INVALID_MEMORY_INDEX_TYPE: u16 = 6006;

// Safety error codes (7000-7999)
/// Safety violation error
pub const SAFETY_VIOLATION: u16 = 7000;
/// Safety ASIL violation error
pub const SAFETY_ASIL_VIOLATION: u16 = 7001;
/// Memory corruption detected error
pub const MEMORY_CORRUPTION_DETECTED: u16 = 7002;
/// Safety verification failed error
pub const VERIFICATION_FAILED: u16 = 7003;
/// Safety context invalid error
pub const SAFETY_CONTEXT_INVALID: u16 = 7004;
/// Safety guard failure error
pub const SAFETY_GUARD_FAILURE: u16 = 7005;
/// Determinism violation error (ASIL-D)
pub const DETERMINISM_VIOLATION: u16 = 7006;
/// Redundancy check failure error (ASIL-D)
pub const REDUNDANCY_CHECK_FAILURE: u16 = 7007;
/// ASIL level mismatch error
pub const ASIL_LEVEL_MISMATCH: u16 = 7008;
/// Safety monitor timeout error
pub const SAFETY_MONITOR_TIMEOUT: u16 = 7009;

// Unified types error codes (8000-8999)
/// Unified type configuration error
pub const UNIFIED_TYPE_CONFIG_ERROR: u16 = 8000;
/// Platform capacity mismatch error
pub const PLATFORM_CAPACITY_MISMATCH: u16 = 8001;
/// Type system initialization error
pub const TYPE_SYSTEM_INIT_ERROR: u16 = 8002;
/// Memory provider creation error
pub const MEMORY_PROVIDER_CREATION_ERROR: u16 = 8003;

// Memory system error codes (9000-9999)
/// Memory allocation failed error
pub const MEMORY_ALLOCATION_FAILED: u16 = 9000;
/// Memory deallocation failed error  
pub const MEMORY_DEALLOCATION_FAILED: u16 = 9001;
/// Memory provider capacity exceeded error
pub const MEMORY_PROVIDER_CAPACITY_EXCEEDED: u16 = 9002;
/// Memory provider invalid error
pub const MEMORY_PROVIDER_INVALID: u16 = 9003;
/// Memory provider not found error
pub const MEMORY_PROVIDER_NOT_FOUND: u16 = 9004;
/// Memory alignment error
pub const MEMORY_ALIGNMENT_ERROR: u16 = 9005;

// Foundation types error codes (10000-10999)
/// Bounded collection capacity exceeded error
pub const BOUNDED_COLLECTION_CAPACITY_EXCEEDED: u16 = 10000;
/// Bounded collection invalid capacity error
pub const BOUNDED_COLLECTION_INVALID_CAPACITY: u16 = 10001;
/// Bounded collection conversion error
pub const BOUNDED_COLLECTION_CONVERSION_ERROR: u16 = 10002;
/// Bounded collection slice error
pub const BOUNDED_COLLECTION_SLICE_ERROR: u16 = 10003;
/// Bounded collection UTF-8 error
pub const BOUNDED_COLLECTION_UTF8_ERROR: u16 = 10004;
/// Bounded collection item too large error
pub const BOUNDED_COLLECTION_ITEM_TOO_LARGE: u16 = 10005;
/// Bounded collection verification error
pub const BOUNDED_COLLECTION_VERIFICATION_ERROR: u16 = 10006;
/// Deprecated API usage error
pub const DEPRECATED_API: u16 = 10007;

// Additional error codes for existing categories
/// Invalid value error (general)
pub const INVALID_VALUE: u16 = 1019;
/// Unimplemented feature error
pub const UNIMPLEMENTED: u16 = 1020;
/// Conversion error (general) - moved to system error codes section as
/// `CONVERSION_ERROR`: u16 = 8002
// WIT parsing error codes (11000-11999)
/// WIT input too large error
pub const WIT_INPUT_TOO_LARGE: u16 = 11000;
/// WIT world limit exceeded error  
pub const WIT_WORLD_LIMIT_EXCEEDED: u16 = 11001;
/// WIT interface limit exceeded error
pub const WIT_INTERFACE_LIMIT_EXCEEDED: u16 = 11002;
/// WIT identifier too long error
pub const WIT_IDENTIFIER_TOO_LONG: u16 = 11003;
/// WIT parsing buffer overflow error
pub const WIT_PARSING_BUFFER_OVERFLOW: u16 = 11004;

// Component error codes (12000-12999)
/// Insufficient memory for component error
pub const INSUFFICIENT_MEMORY: u16 = 12000;
/// Component limit exceeded error
pub const COMPONENT_LIMIT_EXCEEDED: u16 = 12001;
/// Resource type limit exceeded error
pub const RESOURCE_TYPE_LIMIT_EXCEEDED: u16 = 12002;
/// Component memory budget exceeded error
pub const COMPONENT_MEMORY_BUDGET_EXCEEDED: u16 = 12003;

// Platform error codes (13000-13999)
/// Platform detection failed error
pub const PLATFORM_DETECTION_FAILED: u16 = 13000;
/// Platform limits discovery failed error
pub const PLATFORM_LIMITS_DISCOVERY_FAILED: u16 = 13001;
/// Memory limit exceeded error
pub const MEMORY_LIMIT_EXCEEDED: u16 = 13002;
/// Stack limit exceeded error
pub const STACK_LIMIT_EXCEEDED: u16 = 13003;
/// Debug infrastructure error
pub const DEBUG_INFRASTRUCTURE_ERROR: u16 = 13004;

// Runtime error codes (14000-14999)
/// CFI validation failed error  
pub const CFI_VALIDATION_FAILED: u16 = 14000;
/// CFI unsupported error
pub const CFI_UNSUPPORTED: u16 = 14001;
/// Execution engine error
pub const EXECUTION_ENGINE_ERROR: u16 = 14002;
/// Memory adapter error
pub const MEMORY_ADAPTER_ERROR: u16 = 14003;
/// Parse invalid global index type error
pub const PARSE_INVALID_GLOBAL_INDEX_TYPE: u16 = 6007;
/// Value out of range for target type
pub const VALUE_OUT_OF_RANGE: u16 = 6015;
/// Type invalid conversion
pub const TYPE_INVALID_CONVERSION: u16 = 6016;

// Security error codes (15000-15999)
/// Access denied error
pub const ACCESS_DENIED: u16 = 15000;
/// Operation not permitted error
pub const OPERATION_NOT_PERMITTED: u16 = 15001;
/// Invalid parameter error
pub const INVALID_PARAMETER: u16 = 15002;
/// Out of bounds error
pub const OUT_OF_BOUNDS: u16 = 15003;
/// Bounds violation error
pub const BOUNDS_VIOLATION: u16 = 15004;
/// Verification required error
pub const VERIFICATION_REQUIRED: u16 = 15005;
/// Type parameter count mismatch
pub const TYPE_PARAM_COUNT_MISMATCH: u16 = 6017;
/// Type parameter type mismatch
pub const TYPE_PARAM_TYPE_MISMATCH: u16 = 6018;
/// Type result count mismatch
pub const TYPE_RESULT_COUNT_MISMATCH: u16 = 6019;
/// Type result type mismatch
pub const TYPE_RESULT_TYPE_MISMATCH: u16 = 6020;
/// Invalid byte length for a given type or operation
pub const INVALID_BYTE_LENGTH: u16 = 6021;
/// Capacity of a bounded collection (e.g., `BoundedVec`, `BoundedString`) was
/// exceeded during an operation like push or extend.
pub const BOUNDED_COLLECTION_CAPACITY: u16 = 6022;

// Runtime error codes (7000-7999)
/// Runtime error
pub const RUNTIME_ERROR: u16 = 7000;
/// Execution timeout error
pub const EXECUTION_TIMEOUT: u16 = 7001;
/// Fuel exhausted error
pub const FUEL_EXHAUSTED: u16 = 7002;
/// Poisoned lock error
pub const POISONED_LOCK: u16 = 7003;
/// Runtime memory integrity error
pub const RUNTIME_MEMORY_INTEGRITY_ERROR: u16 = 7004;
/// Runtime stack integrity error
pub const RUNTIME_STACK_INTEGRITY_ERROR: u16 = 7005;
/// Runtime label integrity error
pub const RUNTIME_LABEL_INTEGRITY_ERROR: u16 = 7006;
/// Runtime frame integrity error
pub const RUNTIME_FRAME_INTEGRITY_ERROR: u16 = 7007;

// System error codes (8000-8999)
/// System error
pub const SYSTEM_ERROR: u16 = 8000;
/// Unsupported operation error
pub const UNSUPPORTED_OPERATION: u16 = 8001;
/// Conversion error
pub const CONVERSION_ERROR: u16 = 8002;
/// Decoding error
pub const DECODING_ERROR: u16 = 8003;
/// Concurrency error
pub const CONCURRENCY_LOCK_FAILURE: u16 = 8004;
/// Initialization failure
pub const CONCURRENCY_INITIALIZATION_FAILURE: u16 = 8005;
/// Capacity limit exceeded
pub const CAPACITY_LIMIT_EXCEEDED: u16 = 8006;
/// Serialization error
pub const SERIALIZATION_ERROR: u16 = 8007;
/// Deserialization error
pub const DESERIALIZATION_ERROR: u16 = 8008;
/// System call interrupted error
pub const SYSTEM_CALL_INTERRUPTED: u16 = 8009;
/// Generic concurrency error
pub const CONCURRENCY_ERROR: u16 = 8010;
/// Implementation defined limit was exceeded
pub const IMPLEMENTATION_LIMIT: u16 = 8011;
/// Buffer provided is too small for the operation
pub const BUFFER_TOO_SMALL: u16 = 8012;
/// Operation attempted on an object in an unexpected or invalid state
pub const UNEXPECTED_STATE: u16 = 8013;

// Unknown error code
// Global memory system error codes (9500-9599)
/// Duplicate operation attempted
pub const DUPLICATE_OPERATION: u16 = 9500;
/// System or component not initialized
pub const UNINITIALIZED: u16 = 9501;

// Async error codes (9600-9699)
/// Generic async error
pub const ASYNC_ERROR: u16 = 9600;
/// Async task cancelled
pub const ASYNC_CANCELLED: u16 = 9601;
/// Async deadlock detected
pub const ASYNC_DEADLOCK: u16 = 9602;
/// Async task panicked
pub const ASYNC_PANIC: u16 = 9603;
/// Async stream closed
pub const ASYNC_STREAM_CLOSED: u16 = 9604;
/// Async operation timeout
pub const ASYNC_TIMEOUT: u16 = 9605;

/// Out of memory error
pub const OUT_OF_MEMORY: u16 = 9010;

/// I/O error
pub const IO_ERROR: u16 = 8801;

/// Unknown error
pub const UNKNOWN: u16 = 9999;

// Parser error codes (8100-8199)
/// Parse invalid magic bytes error
pub const PARSE_INVALID_MAGIC_BYTES: u16 = 8101;
/// Parse invalid version bytes error
pub const PARSE_INVALID_VERSION_BYTES: u16 = 8102;
/// Parse invalid section ID error
pub const PARSE_INVALID_SECTION_ID: u16 = 8103;
/// Parse invalid local count error
pub const PARSE_INVALID_LOCAL_COUNT: u16 = 8108;
/// Parse invalid label count error
pub const PARSE_INVALID_LABEL_COUNT: u16 = 8109;
/// Parse invalid type definition error
pub const PARSE_INVALID_TYPE_DEF: u16 = 8110;
/// Parse invalid data definition error
pub const PARSE_INVALID_DATA_DEF: u16 = 8111;
/// Parse invalid element definition error
pub const PARSE_INVALID_ELEMENT_DEF: u16 = 8112;
/// Parse invalid value type byte error
pub const PARSE_INVALID_VALTYPE_BYTE: u16 = 8113;
/// Parse invalid opcode byte error
pub const PARSE_INVALID_OPCODE_BYTE: u16 = 8114;
/// Parse invalid LEB128 encoding error
pub const PARSE_INVALID_LEB128_ENCODING: u16 = 8115;
/// Parse unexpected EOF error
pub const PARSE_UNEXPECTED_EOF: u16 = 8116;
/// Parse malformed UTF-8 string error
pub const PARSE_MALFORMED_UTF8_STRING: u16 = 8117;
/// Invalid UTF-8 encoding error
pub const INVALID_UTF8_ENCODING: u16 = 8118;
/// Parse invalid alignment value error
pub const PARSE_INVALID_ALIGNMENT_VALUE: u16 = 8119;
/// Parse invalid reference type byte error
pub const PARSE_INVALID_REFERENCE_TYPE_BYTE: u16 = 8120;

// Validation error codes (8200-8299)
/// Validation memory type mismatch error
pub const VALIDATION_MEMORY_TYPE_MISMATCH_ERROR: u16 = 8205;
/// Validation table type mismatch error
pub const VALIDATION_TABLE_TYPE_MISMATCH_ERROR: u16 = 8206;
/// Validation value type error
pub const VALIDATION_VALUE_TYPE_ERROR: u16 = 8207;
/// Validation stack overflow error
pub const VALIDATION_STACK_OVERFLOW_ERROR: u16 = 8209;
/// Validation type mismatch error
pub const VALIDATION_TYPE_MISMATCH_ERROR: u16 = 8210;
/// Validation control flow error
pub const VALIDATION_CONTROL_FLOW_ERROR: u16 = 8211;
/// Validation branch target error
pub const VALIDATION_BRANCH_TARGET_ERROR: u16 = 8212;
/// Validation unreachable code error
pub const VALIDATION_UNREACHABLE_CODE_ERROR: u16 = 8213;
/// Validation memory access error
pub const VALIDATION_MEMORY_ACCESS_ERROR: u16 = 8214;
/// Validation start function error
pub const VALIDATION_START_FUNCTION_ERROR: u16 = 8215;

// Memory errors (8400-8499)
/// General memory error
pub const MEMORY_ERROR: u16 = 8400;
/// Memory allocation error
pub const MEMORY_ALLOCATION_ERROR: u16 = 8403;
/// Memory grow failure error
pub const MEMORY_GROW_FAILURE: u16 = 8404;
/// Memory alignment error code
pub const MEMORY_ALIGNMENT_ERROR_CODE: u16 = 8405;
/// Memory size limit error
pub const MEMORY_SIZE_LIMIT_ERROR: u16 = 8406;
/// Memory deallocation error
pub const MEMORY_DEALLOCATION_ERROR: u16 = 8407;

// Runtime trap errors (8600-8699)
/// Runtime trap error
pub const RUNTIME_TRAP_ERROR: u16 = 8601;
/// Runtime uninitialized element error
pub const RUNTIME_UNINITIALIZED_ELEMENT_ERROR: u16 = 8602;
/// Runtime unimplemented instruction error
pub const RUNTIME_UNIMPLEMENTED_INSTRUCTION_ERROR: u16 = 8603;
/// Runtime invalid conversion error
pub const RUNTIME_INVALID_CONVERSION_ERROR: u16 = 8604;
/// Runtime division by zero error
pub const RUNTIME_DIVISION_BY_ZERO_ERROR: u16 = 8605;
/// Division by zero error alias
pub const DIVISION_BY_ZERO: u16 = RUNTIME_DIVISION_BY_ZERO_ERROR;
/// Invalid memory index alias
pub const INVALID_MEMORY_INDEX: u16 = VALIDATION_INVALID_MEMORY_INDEX;
/// Invalid data segment index alias
pub const INVALID_DATA_SEGMENT_INDEX: u16 = VALIDATION_INVALID_DATA_SEGMENT_INDEX;
/// Runtime integer overflow error
pub const RUNTIME_INTEGER_OVERFLOW_ERROR: u16 = 8606;
/// Integer overflow error alias
pub const INTEGER_OVERFLOW: u16 = RUNTIME_INTEGER_OVERFLOW_ERROR;
/// Runtime function not found error
pub const RUNTIME_FUNCTION_NOT_FOUND_ERROR: u16 = 8607;
/// Runtime import not found error
pub const RUNTIME_IMPORT_NOT_FOUND_ERROR: u16 = 8608;
/// Runtime memory integrity violation error
pub const RUNTIME_MEMORY_INTEGRITY_VIOLATION: u16 = 8609;
/// Runtime call indirect type mismatch error
pub const RUNTIME_CALL_INDIRECT_TYPE_MISMATCH_ERROR: u16 = 8610;
/// Runtime invalid argument error
pub const RUNTIME_INVALID_ARGUMENT_ERROR: u16 = 8611;
/// Runtime export not found error
pub const RUNTIME_EXPORT_NOT_FOUND_ERROR: u16 = 8612;
/// WASI capability unavailable
pub const WASI_CAPABILITY_UNAVAILABLE: u16 = 8613;
/// Runtime capacity error code
pub const RUNTIME_CAPACITY_ERROR_CODE: u16 = 8614;

// System errors (8800-8899)
/// System IO error code
pub const SYSTEM_IO_ERROR_CODE: u16 = 8801;
/// System resource limit error
pub const SYSTEM_RESOURCE_LIMIT_ERROR: u16 = 8802;
/// System unsupported feature error
pub const SYSTEM_UNSUPPORTED_FEATURE_ERROR: u16 = 8803;

// Security errors (8900-8999)
/// Control Flow Integrity violation
pub const CFI_VIOLATION: u16 = 8900;

// Component errors (9000-9099)
/// Component invalid type error
pub const COMPONENT_INVALID_TYPE_ERROR: u16 = 9001;
/// Component export not found error
pub const COMPONENT_EXPORT_NOT_FOUND_ERROR: u16 = 9002;
/// Component import not found error
pub const COMPONENT_IMPORT_NOT_FOUND_ERROR: u16 = 9003;
/// Component conversion error code
pub const COMPONENT_CONVERSION_ERROR_CODE: u16 = 9005;
/// Component invalid state error
pub const COMPONENT_INVALID_STATE_ERROR: u16 = 9007;
/// Component resource limit error
pub const COMPONENT_RESOURCE_LIMIT_ERROR: u16 = 9008;

/// Mutex error
pub const MUTEX_ERROR: u16 = 7010;

/// Function not found error
pub const FUNCTION_NOT_FOUND: u16 = 2010;

/// Component not found error
pub const COMPONENT_NOT_FOUND: u16 = 2011;

/// Too many components error
pub const TOO_MANY_COMPONENTS: u16 = 2012;

/// Component error
pub const COMPONENT_ERROR: u16 = 2013;

/// WIT parse error
pub const WIT_PARSE_ERROR: u16 = 2014;

/// Invalid input error
pub const INVALID_INPUT: u16 = 2015;

/// Unsupported operation
pub const UNSUPPORTED: u16 = 2016;

/// No WIT definitions found
pub const NO_WIT_DEFINITIONS_FOUND: u16 = 2017;

/// Unsupported WASM 2.0 feature error
pub const UNSUPPORTED_WASM20_FEATURE_ERROR: u16 = 2018;

/// Invalid reference type usage error
pub const INVALID_REFERENCE_TYPE_USAGE_ERROR: u16 = 2019;

/// Bulk operation error
pub const BULK_OPERATION_ERROR: u16 = 2020;

/// SIMD operation error
pub const SIMD_OPERATION_ERROR: u16 = 2021;

/// Tail call error
pub const TAIL_CALL_ERROR: u16 = 2022;

/// Debug info error
pub const DEBUG_INFO_ERROR: u16 = 2023;

/// Would block error
pub const WOULD_BLOCK: u16 = 2024;

/// Platform error
pub const PLATFORM_ERROR: u16 = 2025;

/// Invalid configuration error
pub const INVALID_CONFIG: u16 = 2026;

/// Task not found error
pub const TASK_NOT_FOUND: u16 = 2027;

/// Component already exists error
pub const COMPONENT_ALREADY_EXISTS: u16 = 2028;

/// Insufficient data error
pub const INSUFFICIENT_DATA: u16 = 2029;

// INVALID_STATE already defined above as 5003

// Additional missing error codes
/// Invalid binary format error
pub const INVALID_BINARY: u16 = 8200;

/// Null reference error
pub const NULL_REFERENCE: u16 = 8201;

// RESOURCE_EXHAUSTED already defined above as 3008

// INVALID_ARGUMENT already defined above as 5002

// Component Runtime error codes (24000-24999)
/// Component thread spawn failed
pub const COMPONENT_THREAD_SPAWN_FAILED: u16 = 24000;
/// Component handle representation error
pub const COMPONENT_HANDLE_REPRESENTATION_ERROR: u16 = 24001;
/// Component resource lifecycle error
pub const COMPONENT_RESOURCE_LIFECYCLE_ERROR: u16 = 24002;
/// Component instantiation runtime error
pub const COMPONENT_INSTANTIATION_RUNTIME_ERROR: u16 = 24003;
/// Component ABI runtime error
pub const COMPONENT_ABI_RUNTIME_ERROR: u16 = 24004;
/// Component virtualization error
pub const COMPONENT_VIRTUALIZATION_ERROR: u16 = 24005;
/// Component capability denied
pub const COMPONENT_CAPABILITY_DENIED: u16 = 24006;
/// Component thread join failed
pub const COMPONENT_THREAD_JOIN_FAILED: u16 = 24007;
/// Component thread not found
pub const COMPONENT_THREAD_NOT_FOUND: u16 = 24008;
/// Component configuration invalid
pub const COMPONENT_CONFIGURATION_INVALID: u16 = 24009;

// Platform Runtime error codes (25000-25999)
/// Platform memory allocation failed
pub const PLATFORM_MEMORY_ALLOCATION_FAILED: u16 = 25000;
/// Platform thread creation failed
pub const PLATFORM_THREAD_CREATION_FAILED: u16 = 25001;
/// Platform sync primitive failed
pub const PLATFORM_SYNC_PRIMITIVE_FAILED: u16 = 25002;
/// Platform hardware acceleration failed
pub const PLATFORM_HARDWARE_ACCELERATION_FAILED: u16 = 25003;
/// Platform realtime constraint violated
pub const PLATFORM_REALTIME_CONSTRAINT_VIOLATED: u16 = 25004;
/// Platform page allocator failed
pub const PLATFORM_PAGE_ALLOCATOR_FAILED: u16 = 25005;
/// Platform memory protection failed
pub const PLATFORM_MEMORY_PROTECTION_FAILED: u16 = 25006;
/// Platform watchdog timeout
pub const PLATFORM_WATCHDOG_TIMEOUT: u16 = 25007;
/// Platform IPC failed
pub const PLATFORM_IPC_FAILED: u16 = 25008;

// Foundation Runtime error codes (26000-26999)
/// Foundation bounded capacity exceeded
pub const FOUNDATION_BOUNDED_CAPACITY_EXCEEDED: u16 = 26000;
/// Foundation memory provider failed
pub const FOUNDATION_MEMORY_PROVIDER_FAILED: u16 = 26001;
/// Foundation safety constraint violated
pub const FOUNDATION_SAFETY_CONSTRAINT_VIOLATED: u16 = 26002;
/// Foundation verification failed
pub const FOUNDATION_VERIFICATION_FAILED: u16 = 26003;
/// Foundation allocation budget exceeded
pub const FOUNDATION_ALLOCATION_BUDGET_EXCEEDED: u16 = 26004;
/// Foundation capability verification failed
pub const FOUNDATION_CAPABILITY_VERIFICATION_FAILED: u16 = 26005;
/// Foundation checksum mismatch
pub const FOUNDATION_CHECKSUM_MISMATCH: u16 = 26006;
/// Foundation memory coordination failed
pub const FOUNDATION_MEMORY_COORDINATION_FAILED: u16 = 26007;

// Async Runtime error codes (27000-27999)
/// Async task spawn failed
pub const ASYNC_TASK_SPAWN_FAILED: u16 = 27000;
/// Async fuel exhausted
pub const ASYNC_FUEL_EXHAUSTED: u16 = 27001;
/// Async deadline exceeded
pub const ASYNC_DEADLINE_EXCEEDED: u16 = 27002;
/// Async channel full
pub const ASYNC_CHANNEL_FULL: u16 = 27003;
/// Async channel closed
pub const ASYNC_CHANNEL_CLOSED: u16 = 27004;
/// Async priority inheritance failed
pub const ASYNC_PRIORITY_INHERITANCE_FAILED: u16 = 27005;
/// Async WCET analysis failed
pub const ASYNC_WCET_ANALYSIS_FAILED: u16 = 27006;
/// Async preemption failed
pub const ASYNC_PREEMPTION_FAILED: u16 = 27007;
/// Async resource cleanup failed
pub const ASYNC_RESOURCE_CLEANUP_FAILED: u16 = 27008;

/// Codes representing WebAssembly runtime trap conditions.
/// These are used when an operation cannot complete normally due to a runtime
/// error defined by the WebAssembly specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)] // Optional: if we want to map them to specific numeric codes easily
pub enum TrapCode {
    /// An `unreachable` instruction was executed.
    Unreachable = 1,
    /// Call to an indirect function with an out-of-bounds index.
    IndirectCallIndexOutOfBounds = 2,
    /// Indirect call to a null table entry.
    IndirectCallNullTableEntry = 3,
    /// Indirect call signature mismatch.
    IndirectCallSignatureMismatch = 4,
    /// Integer division by zero.
    IntegerDivideByZero = 5,
    /// Integer overflow during conversion from a float, or float is
    /// NaN/Infinity.
    InvalidConversionToInteger = 6,
    /// Integer overflow for an operation that traps on overflow (e.g.
    /// `i32.div_s` specific case).
    IntegerOverflow = 7,
    /// Out-of-bounds memory access.
    MemoryOutOfBounds = 8,
    /// Attempt to grow memory beyond its limit.
    MemoryGrowOutOfBounds = 9, // Not strictly a trap, but a runtime error condition
    /// Uninitialized element in a table.
    UninitializedElement = 10,
    /// Out-of-bounds table access (e.g. `table.get`, `table.set`).
    TableOutOfBounds = 11,
    // Add more specific trap codes as needed based on Wasm spec.
    /// A generic trap for conditions not covered by more specific codes.
    GenericTrap = 12,
}

impl TrapCode {
    /// Provides a default message for the trap code.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::Unreachable => "unreachable instruction executed",
            Self::IndirectCallIndexOutOfBounds => "indirect call index out of bounds",
            Self::IndirectCallNullTableEntry => "indirect call to null table entry",
            Self::IndirectCallSignatureMismatch => "indirect call signature mismatch",
            Self::IntegerDivideByZero => "integer divide by zero",
            Self::InvalidConversionToInteger => "invalid conversion to integer",
            Self::IntegerOverflow => "integer overflow",
            Self::MemoryOutOfBounds => "out of bounds memory access",
            Self::MemoryGrowOutOfBounds => {
                "failed to grow memory; limit reached or allocation failed"
            },
            Self::UninitializedElement => "uninitialized element",
            Self::TableOutOfBounds => "out of bounds table access",
            Self::GenericTrap => "a WebAssembly trap occurred",
        }
    }
}

// It might also be useful to have a way to convert TrapCode into a general
// Error This is a sketch and might need adjustment based on how Error is
// structured. Assuming Error::new takes an ErrorCategory, a code (we can use
// TrapCode as u16), and a message.
impl From<TrapCode> for crate::Error {
    fn from(trap_code: TrapCode) -> Self {
        Self::new(
            crate::ErrorCategory::RuntimeTrap,
            trap_code as u16, // Use the discriminant value as the code
            trap_code.message(), /* trap_code.message() returns &'static str which fulfills
                               * Into<String> */
        )
    }
}
