//! Tests for error Display implementations
//! Ensures all error types format correctly and don't panic

#[cfg(test)]
mod tests {
    use wrt_error::kinds::*;

    #[test]
    fn test_all_error_type_display_implementations() {
        // Test every error struct's Display implementation

        // Basic error types
        let invalid_type = InvalidType("test_type");
        assert!(!invalid_type.to_string().is_empty());
        assert!(invalid_type.to_string().contains("test_type"));

        let bounds_error = OutOfBoundsError("index 42");
        assert!(!bounds_error.to_string().is_empty());
        assert!(bounds_error.to_string().contains("index 42"));

        let parse_error = ParseError("invalid syntax");
        assert!(!parse_error.to_string().is_empty());
        assert!(parse_error.to_string().contains("invalid syntax"));

        let validation_error = ValidationError("constraint violated");
        assert!(!validation_error.to_string().is_empty());
        assert!(validation_error.to_string().contains("constraint violated"));

        let resource_error = ResourceError("resource exhausted");
        assert!(!resource_error.to_string().is_empty());
        assert!(resource_error.to_string().contains("resource exhausted"));

        let runtime_error = RuntimeError("execution failed");
        assert!(!runtime_error.to_string().is_empty());
        assert!(runtime_error.to_string().contains("execution failed"));

        let component_error = ComponentError("component invalid");
        assert!(!component_error.to_string().is_empty());
        assert!(component_error.to_string().contains("component invalid"));

        let memory_error = MemoryAccessError("out of bounds access");
        assert!(!memory_error.to_string().is_empty());
        assert!(memory_error.to_string().contains("out of bounds access"));

        let lock_error = PoisonedLockError("lock poisoned");
        assert!(!lock_error.to_string().is_empty());
        assert!(lock_error.to_string().contains("lock poisoned"));
    }

    #[test]
    fn test_specific_error_types_display() {
        // Test specific error types that have specialized Display implementations

        let stack_underflow = StackUnderflowError("stack empty");
        assert!(!stack_underflow.to_string().is_empty());

        let export_not_found = ExportNotFoundError("missing_function");
        let display = export_not_found.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Export not found"));
        assert!(display.contains("missing_function"));

        let invalid_instance = InvalidInstanceIndexError(42);
        let display = invalid_instance.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid instance index"));
        assert!(display.contains("42"));

        let invalid_function = InvalidFunctionIndexError(123);
        let display = invalid_function.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid function index"));
        assert!(display.contains("123"));

        let invalid_element = InvalidElementIndexError(456);
        let display = invalid_element.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid element index"));
        assert!(display.contains("456"));

        let invalid_memory = InvalidMemoryIndexError(789);
        let display = invalid_memory.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid memory index"));
        assert!(display.contains("789"));

        let invalid_global = InvalidGlobalIndexError(101);
        let display = invalid_global.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid global index"));
        assert!(display.contains("101"));

        let invalid_data_segment = InvalidDataSegmentIndexError(202);
        let display = invalid_data_segment.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid data segment index"));
        assert!(display.contains("202"));

        let invalid_function_type = InvalidFunctionTypeError("wrong signature");
        assert!(!invalid_function_type.to_string().is_empty());

        let not_implemented = NotImplementedError("feature X");
        assert!(!not_implemented.to_string().is_empty());

        let invalid_value = InvalidValue("NaN");
        assert!(!invalid_value.to_string().is_empty());

        let value_out_of_range = ValueOutOfRangeError("value too large");
        assert!(!value_out_of_range.to_string().is_empty());

        let invalid_state = InvalidState("corrupted");
        assert!(!invalid_state.to_string().is_empty());

        let decoding_error = DecodingError("malformed data");
        assert!(!decoding_error.to_string().is_empty());

        let execution_limit_exceeded = ExecutionLimitExceeded("timeout");
        assert!(!execution_limit_exceeded.to_string().is_empty());

        let execution_timeout = ExecutionTimeoutError("5 seconds");
        assert!(!execution_timeout.to_string().is_empty());

        let resource_limit_exceeded = ResourceLimitExceeded("memory full");
        assert!(!resource_limit_exceeded.to_string().is_empty());

        let invalid_argument = InvalidArgumentError("null pointer");
        assert!(!invalid_argument.to_string().is_empty());
    }

    #[test]
    fn test_complex_error_types_display() {
        // Test error types with more complex Display logic

        let wasm30_construct = UnsupportedWasm30ConstructInWasm20Module {
            construct_name: "advanced_feature",
        };
        let display = wasm30_construct.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Unsupported Wasm 3.0 construct"));
        assert!(display.contains("advanced_feature"));

        let wasm30_instruction = InvalidWasm30InstructionImmediate {
            instruction: "complex.instr",
        };
        let display = wasm30_instruction.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid Wasm 3.0 instruction immediate"));
        assert!(display.contains("complex.instr"));

        let malformed_type_info = MalformedWasm30TypeInformationSection("corrupt data");
        let display = malformed_type_info.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Malformed Wasm 3.0 TypeInformation section"));
        assert!(display.contains("corrupt data"));

        let invalid_memory_wasm30 = InvalidMemoryIndexWasm30 {
            index: 5,
            max_memories: 3,
        };
        let display = invalid_memory_wasm30.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid Wasm 3.0 memory index"));
        assert!(display.contains('5'));
        assert!(display.contains('3'));

        let unknown_opcode = UnknownOpcodeForVersion {
            version_major: 2,
            version_minor: 1,
            opcode_byte1: 0xAB,
            opcode_byte2: Some(0xCD),
        };
        let display = unknown_opcode.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Unknown opcode for Wasm 2.1"));
        assert!(display.contains("0xAB"));
        assert!(display.contains("0xCD"));

        let unknown_opcode_no_byte2 = UnknownOpcodeForVersion {
            version_major: 1,
            version_minor: 0,
            opcode_byte1: 0x12,
            opcode_byte2: None,
        };
        let display = unknown_opcode_no_byte2.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Unknown opcode for Wasm 1.0"));
        assert!(display.contains("0x12"));
        assert!(!display.contains("byte2"));

        let invalid_import_export = InvalidImportExportKindForVersion {
            version_major: 1,
            version_minor: 1,
            kind_byte: 0xFF,
        };
        let display = invalid_import_export.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid import/export kind for Wasm 1.1"));
        assert!(display.contains("0xFF"));

        let unsupported_wasm20 = UnsupportedWasm20Feature {
            feature_name: "threads",
        };
        let display = unsupported_wasm20.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Unsupported Wasm 2.0 feature"));
        assert!(display.contains("threads"));

        let invalid_ref_type = InvalidReferenceTypeUsage {
            message: "funcref in wrong context",
        };
        let display = invalid_ref_type.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Invalid reference type usage"));
        assert!(display.contains("funcref in wrong context"));

        let bulk_op_error = BulkOperationError {
            operation_name: "memory.copy",
            reason: "overlapping regions",
        };
        let display = bulk_op_error.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Bulk operation error"));
        assert!(display.contains("memory.copy"));
        assert!(display.contains("overlapping regions"));

        let simd_error = SimdOperationError {
            instruction_name: "v128.load",
            reason: "misaligned address",
        };
        let display = simd_error.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("SIMD operation error"));
        assert!(display.contains("v128.load"));
        assert!(display.contains("misaligned address"));

        let tail_call_error = TailCallError {
            message: "stack overflow",
        };
        let display = tail_call_error.to_string();
        assert!(!display.is_empty());
        assert!(display.contains("Tail call error"));
        assert!(display.contains("stack overflow"));
    }

    #[test]
    fn test_error_display_never_panics() {
        // Ensure Display implementations never panic with edge cases

        let empty_string_tests = vec![
            InvalidType("").to_string(),
            OutOfBoundsError("").to_string(),
            ParseError("").to_string(),
            ValidationError("").to_string(),
            ResourceError("").to_string(),
            RuntimeError("").to_string(),
            ComponentError("").to_string(),
            MemoryAccessError("").to_string(),
            PoisonedLockError("").to_string(),
        ];

        for display in empty_string_tests {
            assert!(
                !display.is_empty(),
                "Display should handle empty strings gracefully"
            );
        }

        // Test with special characters
        let special_char_tests = vec![
            InvalidType("test\n\r\t").to_string(),
            OutOfBoundsError("unicode: 🦀").to_string(),
            ParseError("quotes: \"'`").to_string(),
        ];

        for display in special_char_tests {
            assert!(
                !display.is_empty(),
                "Display should handle special characters"
            );
        }
    }
}
