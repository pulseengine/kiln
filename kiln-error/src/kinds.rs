// WRT - wrt-error
// Module: WRT Error Kinds
// SW-REQ-ID: REQ_004
// SW-REQ-ID: REQ_ERROR_001
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

/// Validation error for integrity or consistency checks
#[derive(Debug, Clone)]
pub struct ValidationError(pub &'static str);

/// Out of bounds error for memory access or index violations
#[derive(Debug, Clone)]
pub struct OutOfBoundsError(pub &'static str);

/// Parse error for decoding binary formats
#[derive(Debug, Clone)]
pub struct ParseError(pub &'static str);

/// Type error for type mismatches or invalid types
#[derive(Debug, Clone)]
pub struct InvalidType(pub &'static str);

/// Conversion error
#[derive(Debug, Clone)]
pub struct ConversionError(pub &'static str);

/// Division by zero error
#[derive(Debug, Clone, Copy)]
pub struct DivisionByZeroError;

/// Integer overflow error
#[derive(Debug, Clone, Copy)]
pub struct IntegerOverflowError;

/// Stack underflow error
#[derive(Debug, Clone, Copy)]
pub struct StackUnderflow;

/// Type mismatch error
#[derive(Debug, Clone)]
pub struct TypeMismatch(pub &'static str);

/// Invalid table index error
#[derive(Debug, Clone, Copy)]
pub struct InvalidTableIndexError(pub u32);

/// Invalid local index error
#[derive(Debug, Clone, Copy)]
pub struct InvalidLocalIndexError(pub u32);

/// Resource error for resource access or creation issues
#[derive(Debug, Clone)]
pub struct ResourceError(pub &'static str);

/// Component error for component instantiation or linking issues
#[derive(Debug, Clone)]
pub struct ComponentError(pub &'static str);

/// Runtime error for generic execution issues
#[derive(Debug, Clone)]
pub struct RuntimeError(pub &'static str);

/// Poisoned lock error for mutex failures
#[derive(Debug, Clone)]
pub struct PoisonedLockError(pub &'static str);

/// Memory access out of bounds error
#[derive(Debug, Clone, Copy)]
pub struct MemoryAccessOutOfBoundsError {
    /// The memory address that was accessed
    pub address: u64,
    /// The length of memory being accessed
    pub length: u64,
}

/// Type mismatch error
#[derive(Debug, Clone)]
pub struct TypeMismatchError(pub &'static str);

/// Table access out of bounds error
#[derive(Debug, Clone, Copy)]
pub struct TableAccessOutOfBounds;

/// Arithmetic error for math operations
#[derive(Debug, Clone)]
pub struct ArithmeticError(pub &'static str);

/// Memory access error
#[derive(Debug, Clone)]
pub struct MemoryAccessError(pub &'static str);

/// Resource exhaustion error
#[derive(Debug, Clone)]
pub struct ResourceExhaustionError(pub &'static str);

/// Invalid index error
#[derive(Debug, Clone)]
pub struct InvalidIndexError(pub &'static str);

/// Error during execution
#[derive(Debug, Clone)]
pub struct ExecutionError(pub &'static str);

/// Stack underflow error (empty stack)
#[derive(Debug, Clone)]
pub struct StackUnderflowError(pub &'static str);

/// Export not found error
#[derive(Debug, Clone)]
pub struct ExportNotFoundError(pub &'static str);

/// Invalid instance index error
#[derive(Debug, Clone, Copy)]
pub struct InvalidInstanceIndexError(pub u32);

/// Invalid function index error
#[derive(Debug, Clone, Copy)]
pub struct InvalidFunctionIndexError(pub u32);

/// Invalid element index error
#[derive(Debug, Clone, Copy)]
pub struct InvalidElementIndexError(pub usize);

/// Invalid memory index error
#[derive(Debug, Clone, Copy)]
pub struct InvalidMemoryIndexError(pub u32);

/// Invalid global index error
#[derive(Debug, Clone, Copy)]
pub struct InvalidGlobalIndexError(pub u32);

/// Invalid data segment index error
#[derive(Debug, Clone, Copy)]
pub struct InvalidDataSegmentIndexError(pub usize);

/// Invalid function type error
#[derive(Debug, Clone)]
pub struct InvalidFunctionTypeError(pub &'static str);

/// Not implemented error
#[derive(Debug, Clone)]
pub struct NotImplementedError(pub &'static str);

/// Out of bounds access error
#[derive(Debug, Clone)]
pub struct OutOfBoundsAccess(pub &'static str);

/// Invalid value error
#[derive(Debug, Clone)]
pub struct InvalidValue(pub &'static str);

/// Value out of range error
#[derive(Debug, Clone)]
pub struct ValueOutOfRangeError(pub &'static str);

/// Invalid state error
#[derive(Debug, Clone)]
pub struct InvalidState(pub &'static str);

/// Decoding error
#[derive(Debug, Clone)]
pub struct DecodingError(pub &'static str);

/// Execution limit exceeded error
#[derive(Debug, Clone)]
pub struct ExecutionLimitExceeded(pub &'static str);

/// Execution timeout error
#[derive(Debug, Clone)]
pub struct ExecutionTimeoutError(pub &'static str);

/// Resource limit exceeded error
#[derive(Debug, Clone)]
pub struct ResourceLimitExceeded(pub &'static str);

/// Invalid argument error
#[derive(Debug, Clone)]
pub struct InvalidArgumentError(pub &'static str);

/// Error when a Wasm 3.0 specific construct is encountered in a Wasm 2.0 module
/// or context.
#[derive(Debug, Clone)]
pub struct UnsupportedWasm30ConstructInWasm20Module {
    /// Name or description of the Wasm 3.0 construct.
    pub construct_name: &'static str,
}

/// Error for malformed or invalid immediates for a Wasm 3.0 instruction.
#[derive(Debug, Clone)]
pub struct InvalidWasm30InstructionImmediate {
    /// Name of the Wasm 3.0 instruction.
    pub instruction: &'static str,
}

/// Error for a malformed Wasm 3.0 `TypeInformation` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedWasm30TypeInformationSection(pub &'static str);

/// Error for an invalid memory index used with Wasm 3.0 multi-memory features.
#[derive(Debug, Clone, Copy)]
pub struct InvalidMemoryIndexWasm30 {
    /// The invalid memory index that was used.
    pub index: u32,
    /// The maximum number of allowed memories (if applicable).
    pub max_memories: u32,
}

/// Error for an unknown opcode for the detected/specified Wasm version.
#[derive(Debug, Clone, Copy)]
pub struct UnknownOpcodeForVersion {
    /// Major version number of Wasm (e.g., 2 for Wasm 2.0, 3 for Wasm 3.0).
    pub version_major: u16,
    /// Minor version number of Wasm.
    pub version_minor: u16,
    /// The first byte of the opcode.
    pub opcode_byte1: u8,
    /// The second byte of the opcode, if it's a multi-byte opcode.
    pub opcode_byte2: Option<u8>,
}

/// Error for an invalid import/export kind byte for the detected/specified Wasm
/// version.
#[derive(Debug, Clone, Copy)]
pub struct InvalidImportExportKindForVersion {
    /// Major version number of Wasm.
    pub version_major: u16,
    /// Minor version number of Wasm.
    pub version_minor: u16,
    /// The kind byte that was encountered.
    pub kind_byte: u8,
}

/// Helper function for creating `ValidationError`
#[must_use]
pub const fn validation_error(message: &'static str) -> ValidationError {
    ValidationError(message)
}

/// Helper function for creating `OutOfBoundsError`
#[must_use]
pub const fn out_of_bounds_error(message: &'static str) -> OutOfBoundsError {
    OutOfBoundsError(message)
}

/// Helper function for creating `ParseError`
#[must_use]
pub const fn parse_error(message: &'static str) -> ParseError {
    ParseError(message)
}

/// Helper function for creating `InvalidType`
#[must_use]
pub const fn invalid_type(message: &'static str) -> InvalidType {
    InvalidType(message)
}

/// Helper function for creating `ConversionError`
#[must_use]
pub const fn conversion_error(message: &'static str) -> ConversionError {
    ConversionError(message)
}

/// Helper function for creating `DivisionByZeroError`
#[must_use]
pub const fn division_by_zero_error() -> DivisionByZeroError {
    DivisionByZeroError
}

/// Helper function for creating `IntegerOverflowError`
#[must_use]
pub const fn integer_overflow_error() -> IntegerOverflowError {
    IntegerOverflowError
}

/// Helper function for creating `StackUnderflow`
#[must_use]
pub const fn stack_underflow() -> StackUnderflow {
    StackUnderflow
}

/// Helper function for creating `TypeMismatch`
#[must_use]
pub const fn type_mismatch(message: &'static str) -> TypeMismatch {
    TypeMismatch(message)
}

/// Helper function for creating `InvalidTableIndexError`
#[must_use]
pub const fn invalid_table_index_error(index: u32) -> InvalidTableIndexError {
    InvalidTableIndexError(index)
}

/// Helper function for creating `ResourceError`
#[must_use]
pub const fn resource_error(message: &'static str) -> ResourceError {
    ResourceError(message)
}

/// Helper function for creating `ComponentError`
#[must_use]
pub const fn component_error(message: &'static str) -> ComponentError {
    ComponentError(message)
}

/// Helper function for creating `RuntimeError`
#[must_use]
pub const fn runtime_error(message: &'static str) -> RuntimeError {
    RuntimeError(message)
}

/// Helper function for creating `PoisonedLockError`
#[must_use]
pub const fn poisoned_lock_error(message: &'static str) -> PoisonedLockError {
    PoisonedLockError(message)
}

/// Threading error for thread spawn and management issues
#[derive(Debug, Clone)]
pub struct ThreadingError(pub &'static str);

/// Helper function for creating `ThreadingError`
#[must_use]
pub const fn threading_error(message: &'static str) -> ThreadingError {
    ThreadingError(message)
}

/// Helper function for creating `TypeMismatchError`
#[must_use]
pub const fn type_mismatch_error(message: &'static str) -> TypeMismatchError {
    TypeMismatchError(message)
}

/// Helper function for creating `ArithmeticError`
#[must_use]
pub const fn arithmetic_error(message: &'static str) -> ArithmeticError {
    ArithmeticError(message)
}

/// Helper function for creating `MemoryAccessError`
#[must_use]
pub const fn memory_access_error(message: &'static str) -> MemoryAccessError {
    MemoryAccessError(message)
}

/// Helper function for creating `ResourceExhaustionError`
#[must_use]
pub const fn resource_exhaustion_error(message: &'static str) -> ResourceExhaustionError {
    ResourceExhaustionError(message)
}

/// Helper function for creating `InvalidIndexError`
#[must_use]
pub const fn invalid_index_error(message: &'static str) -> InvalidIndexError {
    InvalidIndexError(message)
}

/// Helper function for creating `ExecutionError`
#[must_use]
pub const fn execution_error(message: &'static str) -> ExecutionError {
    ExecutionError(message)
}

/// Helper function for creating `StackUnderflowError`
#[must_use]
pub const fn stack_underflow_error(message: &'static str) -> StackUnderflowError {
    StackUnderflowError(message)
}

/// Helper function for creating `ExportNotFoundError`
#[must_use]
pub const fn export_not_found_error(name: &'static str) -> ExportNotFoundError {
    ExportNotFoundError(name)
}

/// Helper function for creating `InvalidInstanceIndexError`
#[must_use]
pub const fn invalid_instance_index_error(index: u32) -> InvalidInstanceIndexError {
    InvalidInstanceIndexError(index)
}

/// Helper function for creating `InvalidFunctionIndexError`
#[must_use]
pub const fn invalid_function_index_error(index: u32) -> InvalidFunctionIndexError {
    InvalidFunctionIndexError(index)
}

/// Helper function for creating `InvalidElementIndexError`
#[must_use]
pub const fn invalid_element_index_error(index: usize) -> InvalidElementIndexError {
    InvalidElementIndexError(index)
}

/// Helper function for creating `InvalidMemoryIndexError`
#[must_use]
pub const fn invalid_memory_index_error(index: u32) -> InvalidMemoryIndexError {
    InvalidMemoryIndexError(index)
}

/// Helper function for creating `InvalidGlobalIndexError`
#[must_use]
pub const fn invalid_global_index_error(index: u32) -> InvalidGlobalIndexError {
    InvalidGlobalIndexError(index)
}

/// Helper function for creating `InvalidDataSegmentIndexError`
#[must_use]
pub const fn invalid_data_segment_index_error(index: usize) -> InvalidDataSegmentIndexError {
    InvalidDataSegmentIndexError(index)
}

/// Helper function for creating `InvalidFunctionTypeError`
#[must_use]
pub const fn invalid_function_type_error(message: &'static str) -> InvalidFunctionTypeError {
    InvalidFunctionTypeError(message)
}

/// Helper function for creating `NotImplementedError`
#[must_use]
pub const fn not_implemented_error(message: &'static str) -> NotImplementedError {
    NotImplementedError(message)
}

/// Helper function for creating `OutOfBoundsAccess`
#[must_use]
pub const fn out_of_bounds_access(message: &'static str) -> OutOfBoundsAccess {
    OutOfBoundsAccess(message)
}

/// Helper function for creating `InvalidValue`
#[must_use]
pub const fn invalid_value(message: &'static str) -> InvalidValue {
    InvalidValue(message)
}

/// Helper function for creating `ValueOutOfRangeError`
#[must_use]
pub const fn value_out_of_range_error(message: &'static str) -> ValueOutOfRangeError {
    ValueOutOfRangeError(message)
}

/// Helper function for creating `InvalidState`
#[must_use]
pub const fn invalid_state(message: &'static str) -> InvalidState {
    InvalidState(message)
}

/// Helper function for creating `DecodingError`
#[must_use]
pub const fn decoding_error(message: &'static str) -> DecodingError {
    DecodingError(message)
}

/// Helper function for creating `ExecutionLimitExceeded`
#[must_use]
pub const fn execution_limit_exceeded(message: &'static str) -> ExecutionLimitExceeded {
    ExecutionLimitExceeded(message)
}

/// Helper function for creating `ExecutionTimeoutError`
#[must_use]
pub const fn execution_timeout_error(message: &'static str) -> ExecutionTimeoutError {
    ExecutionTimeoutError(message)
}

/// Helper function for creating `ResourceLimitExceeded`
#[must_use]
pub const fn resource_limit_exceeded(message: &'static str) -> ResourceLimitExceeded {
    ResourceLimitExceeded(message)
}

/// Helper function for creating `InvalidArgumentError`
#[must_use]
pub const fn invalid_argument_error(message: &'static str) -> InvalidArgumentError {
    InvalidArgumentError(message)
}

/// Helper function for creating `UnsupportedWasm30ConstructInWasm20Module`
#[must_use]
pub const fn unsupported_wasm30_construct_in_wasm20_module(
    construct_name: &'static str,
) -> UnsupportedWasm30ConstructInWasm20Module {
    UnsupportedWasm30ConstructInWasm20Module { construct_name }
}

/// Helper function for creating `InvalidWasm30InstructionImmediate`
#[must_use]
pub const fn invalid_wasm30_instruction_immediate(
    instruction: &'static str,
) -> InvalidWasm30InstructionImmediate {
    InvalidWasm30InstructionImmediate { instruction }
}

/// Helper function for creating `MalformedWasm30TypeInformationSection`
#[must_use]
pub const fn malformed_wasm30_type_information_section(
    message: &'static str,
) -> MalformedWasm30TypeInformationSection {
    MalformedWasm30TypeInformationSection(message)
}

/// Helper function for creating `InvalidMemoryIndexWasm30`
#[must_use]
pub const fn invalid_memory_index_wasm30(
    index: u32,
    max_memories: u32,
) -> InvalidMemoryIndexWasm30 {
    InvalidMemoryIndexWasm30 {
        index,
        max_memories,
    }
}

/// Helper function for creating `UnknownOpcodeForVersion`
#[must_use]
pub const fn unknown_opcode_for_version(
    version_major: u16,
    version_minor: u16,
    opcode_byte1: u8,
    opcode_byte2: Option<u8>,
) -> UnknownOpcodeForVersion {
    UnknownOpcodeForVersion {
        version_major,
        version_minor,
        opcode_byte1,
        opcode_byte2,
    }
}

/// Helper function for creating `InvalidImportExportKindForVersion`
#[must_use]
pub const fn invalid_import_export_kind_for_version(
    version_major: u16,
    version_minor: u16,
    kind_byte: u8,
) -> InvalidImportExportKindForVersion {
    InvalidImportExportKindForVersion {
        version_major,
        version_minor,
        kind_byte,
    }
}

/// Error when a Wasm 2.0 specific construct is encountered in a context that
/// does not support it (e.g. Wasm 1.0).
#[derive(Debug, Clone)]
pub struct UnsupportedWasm20Feature {
    /// Name or description of the Wasm 2.0 feature.
    pub feature_name: &'static str,
}

/// Error for malformed or invalid usage of reference types (externref,
/// funcref).
#[derive(Debug, Clone)]
pub struct InvalidReferenceTypeUsage {
    /// Detailed message about the invalid usage.
    pub message: &'static str,
}

/// Error related to Wasm 2.0 bulk memory or table operations.
#[derive(Debug, Clone)]
pub struct BulkOperationError {
    /// Name of the bulk operation instruction (e.g., "memory.copy",
    /// "table.init").
    pub operation_name: &'static str,
    /// Detailed message about the error.
    pub reason: &'static str,
}

/// Error specific to Wasm 2.0 SIMD operations.
#[derive(Debug, Clone)]
pub struct SimdOperationError {
    /// Name of the SIMD instruction.
    pub instruction_name: &'static str,
    /// Detailed message about the SIMD error (e.g., invalid lane index, type
    /// mismatch).
    pub reason: &'static str,
}

/// Error related to Wasm 2.0 tail call instructions.
#[derive(Debug, Clone)]
pub struct TailCallError {
    /// Detailed message about the tail call error.
    pub message: &'static str,
}

/// Creates a new `UnsupportedWasm20Feature` error.
#[must_use]
pub const fn unsupported_wasm20_feature(feature_name: &'static str) -> UnsupportedWasm20Feature {
    UnsupportedWasm20Feature { feature_name }
}

/// Creates a new `InvalidReferenceTypeUsage` error.
#[must_use]
pub const fn invalid_reference_type_usage(message: &'static str) -> InvalidReferenceTypeUsage {
    InvalidReferenceTypeUsage { message }
}

/// Creates a new `BulkOperationError` error.
#[must_use]
pub const fn bulk_operation_error(
    operation_name: &'static str,
    reason: &'static str,
) -> BulkOperationError {
    BulkOperationError {
        operation_name,
        reason,
    }
}

/// Creates a new `SimdOperationError` error.
#[must_use]
pub const fn simd_operation_error(
    instruction_name: &'static str,
    reason: &'static str,
) -> SimdOperationError {
    SimdOperationError {
        instruction_name,
        reason,
    }
}

/// Creates a new `TailCallError` error.
#[must_use]
pub const fn tail_call_error(message: &'static str) -> TailCallError {
    TailCallError { message }
}

impl core::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Validation error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for OutOfBoundsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Out of bounds error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Parse error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for InvalidType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Invalid type")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for DivisionByZeroError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Division by zero")
    }
}

impl core::fmt::Display for IntegerOverflowError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Integer overflow")
    }
}

impl core::fmt::Display for StackUnderflow {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Stack underflow")
    }
}

impl core::fmt::Display for TypeMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for InvalidTableIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid table index: {}", self.0)
    }
}

impl core::fmt::Display for InvalidLocalIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid local index: {}", self.0)
    }
}

impl core::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Resource error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for ComponentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Component error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Runtime error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for PoisonedLockError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Poisoned lock error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for MemoryAccessOutOfBoundsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Memory access out of bounds: address {}, length {}",
            self.address, self.length
        )
    }
}

impl core::fmt::Display for TypeMismatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for TableAccessOutOfBounds {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Table access out of bounds")
    }
}

impl core::fmt::Display for ArithmeticError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for MemoryAccessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Memory access error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl core::fmt::Display for ResourceExhaustionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for InvalidIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for StackUnderflowError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for ExportNotFoundError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Export not found: {}", self.0)
    }
}

impl core::fmt::Display for InvalidInstanceIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid instance index: {}", self.0)
    }
}

impl core::fmt::Display for InvalidFunctionIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid function index: {}", self.0)
    }
}

impl core::fmt::Display for InvalidElementIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid element index: {}", self.0)
    }
}

impl core::fmt::Display for InvalidMemoryIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid memory index: {}", self.0)
    }
}

impl core::fmt::Display for InvalidGlobalIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid global index: {}", self.0)
    }
}

impl core::fmt::Display for InvalidDataSegmentIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid data segment index: {}", self.0)
    }
}

impl core::fmt::Display for InvalidFunctionTypeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for NotImplementedError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for OutOfBoundsAccess {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for InvalidValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for ValueOutOfRangeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for InvalidState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for DecodingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for ExecutionLimitExceeded {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for ExecutionTimeoutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for ResourceLimitExceeded {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for InvalidArgumentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for UnsupportedWasm30ConstructInWasm20Module {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Unsupported Wasm 3.0 construct in Wasm 2.0 module: {}",
            self.construct_name
        )
    }
}

impl core::fmt::Display for InvalidWasm30InstructionImmediate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Invalid Wasm 3.0 instruction immediate: {}",
            self.instruction
        )
    }
}

impl core::fmt::Display for MalformedWasm30TypeInformationSection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Malformed Wasm 3.0 TypeInformation section: {}", self.0)
    }
}

impl core::fmt::Display for InvalidMemoryIndexWasm30 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Invalid Wasm 3.0 memory index: {}, max memories: {}",
            self.index, self.max_memories
        )
    }
}

impl core::fmt::Display for UnknownOpcodeForVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Unknown opcode for Wasm {}.{}: byte1=0x{:02X}",
            self.version_major, self.version_minor, self.opcode_byte1
        )?;
        if let Some(byte2) = self.opcode_byte2 {
            write!(f, ", byte2=0x{byte2:02X}")?;
        }
        Ok(())
    }
}

impl core::fmt::Display for InvalidImportExportKindForVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Invalid import/export kind for Wasm {}.{}: kind_byte=0x{:02X}",
            self.version_major, self.version_minor, self.kind_byte
        )
    }
}

impl core::fmt::Display for UnsupportedWasm20Feature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Unsupported Wasm 2.0 feature: {}", self.feature_name)
    }
}

impl core::fmt::Display for InvalidReferenceTypeUsage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid reference type usage: {}", self.message)
    }
}

impl core::fmt::Display for BulkOperationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Bulk operation error [{}]: {}",
            self.operation_name, self.reason
        )
    }
}

impl core::fmt::Display for SimdOperationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "SIMD operation error [{}]: {}",
            self.instruction_name, self.reason
        )
    }
}

impl core::fmt::Display for TailCallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Tail call error: {}", self.message)
    }
}

impl core::fmt::Display for ThreadingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0.is_empty() {
            write!(f, "Threading error")
        } else {
            write!(f, "{}", self.0)
        }
    }
}
