//! Tests for error constants to improve code coverage
//! This ensures all error constants are referenced and validated

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use wrt_error::codes;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_all_error_constants_are_unique() {
        let mut seen_codes = HashSet::new();

        // Core error codes (1000-1999)
        let core_codes = vec![
            codes::STACK_UNDERFLOW,
            codes::STACK_OVERFLOW,
            codes::UNALIGNED_MEMORY_ACCESS,
            codes::INVALID_MEMORY_ACCESS,
            codes::INVALID_INSTANCE_INDEX,
            codes::EXECUTION_ERROR,
            codes::NOT_IMPLEMENTED,
            codes::MEMORY_ACCESS_ERROR,
            codes::INITIALIZATION_ERROR,
            codes::TYPE_MISMATCH,
            codes::PARSE_ERROR,
            codes::INVALID_VERSION,
            codes::OUT_OF_BOUNDS_ERROR,
            codes::EXECUTION_INSTRUCTION_INDEX_OUT_OF_BOUNDS,
            codes::EXECUTION_INVALID_FRAME,
            codes::EXECUTION_READER_NOT_IMPLEMENTED,
            codes::CAPACITY_EXCEEDED,
            codes::GAS_LIMIT_EXCEEDED,
            codes::CALL_STACK_EXHAUSTED,
            codes::INVALID_VALUE,
            codes::UNIMPLEMENTED,
        ];

        for code in &core_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 1000 && *code < 2000,
                "Core error code {code} out of range"
            );
        }

        // Component model error codes (2000-2999)
        let component_codes = vec![
            codes::INVALID_FUNCTION_INDEX,
            codes::COMPONENT_TYPE_MISMATCH,
            codes::ENCODING_ERROR,
            codes::EXECUTION_LIMIT_EXCEEDED,
            codes::COMPONENT_INSTANTIATION_ERROR,
            codes::CANONICAL_ABI_ERROR,
            codes::COMPONENT_LINKING_ERROR,
        ];

        for code in &component_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 2000 && *code < 3000,
                "Component error code {code} out of range"
            );
        }

        // Resource error codes (3000-3999)
        let resource_codes = vec![
            codes::RESOURCE_ERROR,
            codes::RESOURCE_LIMIT_EXCEEDED,
            codes::RESOURCE_ACCESS_ERROR,
            codes::RESOURCE_NOT_FOUND,
            codes::RESOURCE_INVALID_HANDLE,
            codes::GLOBAL_NOT_FOUND,
            codes::MEMORY_NOT_FOUND,
            codes::TABLE_NOT_FOUND,
        ];

        for code in &resource_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 3000 && *code < 4000,
                "Resource error code {code} out of range"
            );
        }

        // Memory error codes (4000-4999)
        let memory_codes = vec![
            codes::MEMORY_OUT_OF_BOUNDS,
            codes::MEMORY_GROW_ERROR,
            codes::MEMORY_ACCESS_OUT_OF_BOUNDS,
            codes::MEMORY_ACCESS_UNALIGNED,
        ];

        for code in &memory_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 4000 && *code < 5000,
                "Memory error code {code} out of range"
            );
        }

        // Validation error codes (5000-5999)
        let validation_codes = vec![
            codes::VALIDATION_ERROR,
            codes::VALIDATION_FAILURE,
            codes::CHECKSUM_MISMATCH,
            codes::INTEGRITY_VIOLATION,
            codes::VERIFICATION_LEVEL_VIOLATION,
            codes::VALIDATION_GLOBAL_TYPE_MISMATCH,
            codes::VALIDATION_INVALID_MEMORY_INDEX,
            codes::VALIDATION_INVALID_GLOBAL_INDEX,
            codes::VALIDATION_UNSUPPORTED_FEATURE,
            codes::VALIDATION_INVALID_INSTRUCTION,
            codes::VALIDATION_EMPTY_STACK,
            codes::VALIDATION_STACK_SIZE_ERROR,
            codes::VALIDATION_NO_BINARY,
            codes::VALIDATION_FUNCTION_NOT_FOUND,
            codes::VALIDATION_EXPORT_NOT_FOUND,
            codes::VALIDATION_INVALID_FUNCTION_TYPE,
            codes::VALIDATION_INVALID_TABLE_INDEX,
            codes::VALIDATION_INVALID_ELEMENT_INDEX,
            codes::VALIDATION_INVALID_DATA_SEGMENT_INDEX,
            codes::VALIDATION_DUPLICATE_TABLE_REFERENCE,
            codes::VALIDATION_INVALID_FRAME_INDEX,
            codes::VALIDATION_STACK_UNDERFLOW,
            codes::VALIDATION_LIMIT_MIN_EXCEEDS_U32,
            codes::VALIDATION_LIMIT_MAX_EXCEEDS_U32,
            codes::VALIDATION_LIMIT_MAX_LESS_THAN_MIN,
            codes::VALIDATION_INVALID_CUSTOM_SECTION_NAME,
            codes::VALIDATION_CUSTOM_SECTION_DATA_TOO_LONG,
        ];

        for code in &validation_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 5000 && *code < 6000,
                "Validation error code {code} out of range"
            );
        }

        // Type error codes (6000-6999)
        let type_codes = vec![
            codes::INVALID_TYPE,
            codes::TYPE_MISMATCH_ERROR,
            codes::INVALID_FUNCTION_TYPE,
            codes::INVALID_VALUE_TYPE,
            codes::PARSE_INVALID_FUNCTION_INDEX_TYPE,
            codes::PARSE_INVALID_TABLE_INDEX_TYPE,
            codes::PARSE_INVALID_MEMORY_INDEX_TYPE,
            codes::PARSE_INVALID_GLOBAL_INDEX_TYPE,
            codes::VALUE_OUT_OF_RANGE,
            codes::TYPE_INVALID_CONVERSION,
            codes::TYPE_PARAM_COUNT_MISMATCH,
            codes::TYPE_PARAM_TYPE_MISMATCH,
            codes::TYPE_RESULT_COUNT_MISMATCH,
            codes::TYPE_RESULT_TYPE_MISMATCH,
            codes::INVALID_BYTE_LENGTH,
            codes::BOUNDED_COLLECTION_CAPACITY,
        ];

        for code in &type_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 6000 && *code < 7000,
                "Type error code {code} out of range"
            );
        }

        // Runtime error codes (7000-7999)
        let runtime_codes = vec![
            codes::RUNTIME_ERROR,
            codes::EXECUTION_TIMEOUT,
            codes::FUEL_EXHAUSTED,
            codes::POISONED_LOCK,
            codes::RUNTIME_MEMORY_INTEGRITY_ERROR,
            codes::RUNTIME_STACK_INTEGRITY_ERROR,
            codes::RUNTIME_LABEL_INTEGRITY_ERROR,
            codes::RUNTIME_FRAME_INTEGRITY_ERROR,
        ];

        for code in &runtime_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 7000 && *code < 8000,
                "Runtime error code {code} out of range"
            );
        }

        // System error codes (8000-8999)
        let system_codes = vec![
            codes::SYSTEM_ERROR,
            codes::UNSUPPORTED_OPERATION,
            codes::CONVERSION_ERROR,
            codes::DECODING_ERROR,
            codes::CONCURRENCY_LOCK_FAILURE,
            codes::CONCURRENCY_INITIALIZATION_FAILURE,
            codes::CAPACITY_LIMIT_EXCEEDED,
            codes::SERIALIZATION_ERROR,
            codes::DESERIALIZATION_ERROR,
            codes::SYSTEM_CALL_INTERRUPTED,
            codes::CONCURRENCY_ERROR,
            codes::IMPLEMENTATION_LIMIT,
            codes::BUFFER_TOO_SMALL,
            codes::UNEXPECTED_STATE,
        ];

        for code in &system_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 8000 && *code < 9000,
                "System error code {code} out of range"
            );
        }

        // Parser error codes (8100-8199)
        let parser_codes = vec![
            codes::PARSE_INVALID_MAGIC_BYTES,
            codes::PARSE_INVALID_VERSION_BYTES,
            codes::PARSE_INVALID_SECTION_ID,
            codes::PARSE_INVALID_LOCAL_COUNT,
            codes::PARSE_INVALID_LABEL_COUNT,
            codes::PARSE_INVALID_TYPE_DEF,
            codes::PARSE_INVALID_DATA_DEF,
            codes::PARSE_INVALID_ELEMENT_DEF,
            codes::PARSE_INVALID_VALTYPE_BYTE,
            codes::PARSE_INVALID_OPCODE_BYTE,
            codes::PARSE_INVALID_LEB128_ENCODING,
            codes::PARSE_UNEXPECTED_EOF,
            codes::PARSE_MALFORMED_UTF8_STRING,
            codes::PARSE_INVALID_ALIGNMENT_VALUE,
            codes::PARSE_INVALID_REFERENCE_TYPE_BYTE,
        ];

        for code in &parser_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 8100 && *code < 8200,
                "Parser error code {code} out of range"
            );
        }

        // Validation error codes (8200-8299)
        let validation_ext_codes = vec![
            codes::VALIDATION_MEMORY_TYPE_MISMATCH_ERROR,
            codes::VALIDATION_TABLE_TYPE_MISMATCH_ERROR,
            codes::VALIDATION_VALUE_TYPE_ERROR,
            codes::VALIDATION_STACK_OVERFLOW_ERROR,
            codes::VALIDATION_TYPE_MISMATCH_ERROR,
            codes::VALIDATION_CONTROL_FLOW_ERROR,
            codes::VALIDATION_BRANCH_TARGET_ERROR,
        ];

        for code in &validation_ext_codes {
            assert!(seen_codes.insert(*code), "Duplicate error code: {code}");
            assert!(
                *code >= 8200 && *code < 8300,
                "Extended validation error code {code} out of range"
            );
        }

        // Unknown error
        assert!(
            seen_codes.insert(codes::UNKNOWN),
            "Duplicate error code: UNKNOWN"
        );
        assert_eq!(codes::UNKNOWN, 9999);
    }

    #[test]
    fn test_error_code_documentation() {
        // This test ensures all constants are referenced, improving coverage
        // and validates they can be used in match expressions

        fn get_error_description(code: u16) -> &'static str {
            match code {
                codes::STACK_UNDERFLOW => "Stack underflow",
                codes::STACK_OVERFLOW => "Stack overflow",
                codes::UNALIGNED_MEMORY_ACCESS => "Unaligned memory access",
                codes::INVALID_MEMORY_ACCESS => "Invalid memory access",
                codes::INVALID_INSTANCE_INDEX => "Invalid instance index",
                codes::EXECUTION_ERROR => "Execution error",
                codes::NOT_IMPLEMENTED => "Not implemented",
                codes::MEMORY_ACCESS_ERROR => "Memory access error",
                codes::INITIALIZATION_ERROR => "Initialization error",
                codes::TYPE_MISMATCH => "Type mismatch",
                codes::PARSE_ERROR => "Parse error",
                codes::INVALID_VERSION => "Invalid version",
                codes::OUT_OF_BOUNDS_ERROR => "Out of bounds",
                codes::UNKNOWN => "Unknown error",
                _ => "Other error",
            }
        }

        // Test a few codes to ensure the function works
        assert_eq!(
            get_error_description(codes::STACK_UNDERFLOW),
            "Stack underflow"
        );
        assert_eq!(get_error_description(codes::UNKNOWN), "Unknown error");
        assert_eq!(get_error_description(12345), "Other error");
    }
}
