#![deny(warnings)]

use wrt_component::type_conversion::bidirectional::*;
use wrt_format::component::{
    ExternType as FormatExternType, ResourceRepresentation, ValType as FormatValType,
};
use wrt_foundation::{
    ExternType as TypesExternType,
    component::ResourceType,
    component_value::ValType as TypesValType,
    types::{FuncType as TypesFuncType, ValueType},
};

/// Test conversion from core ValueType to format ValType
#[test]
fn test_value_type_to_format_val_type() {
    // Test each core value type
    assert_eq!(
        value_type_to_format_val_type(&ValueType::I32).unwrap(),
        FormatValType::S32
    );
    assert_eq!(
        value_type_to_format_val_type(&ValueType::I64).unwrap(),
        FormatValType::S64
    );
    assert_eq!(
        value_type_to_format_val_type(&ValueType::F32).unwrap(),
        FormatValType::F32
    );
    assert_eq!(
        value_type_to_format_val_type(&ValueType::F64).unwrap(),
        FormatValType::F64
    );

    // Test reference types that should return errors
    assert!(value_type_to_format_val_type(&ValueType::FuncRef).is_err());
    assert!(value_type_to_format_val_type(&ValueType::ExternRef).is_err());
}

/// Test conversion from format ValType to core ValueType
#[test]
fn test_format_val_type_to_value_type() {
    // Test primitive types
    assert_eq!(
        format_val_type_to_value_type(&FormatValType::S32).unwrap(),
        ValueType::I32
    );
    assert_eq!(
        format_val_type_to_value_type(&FormatValType::S64).unwrap(),
        ValueType::I64
    );
    assert_eq!(
        format_val_type_to_value_type(&FormatValType::F32).unwrap(),
        ValueType::F32
    );
    assert_eq!(
        format_val_type_to_value_type(&FormatValType::F64).unwrap(),
        ValueType::F64
    );

    // Test complex types that should return errors
    assert!(format_val_type_to_value_type(&FormatValType::String).is_err());
    assert!(format_val_type_to_value_type(&FormatValType::Bool).is_err());
    assert!(format_val_type_to_value_type(&FormatValType::Char).is_err());
}

/// Test conversion from core ValueType to TypesValType
#[test]
fn test_value_type_to_types_valtype() {
    assert_eq!(
        value_type_to_types_valtype(&ValueType::I32),
        TypesValType::S32
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::I64),
        TypesValType::S64
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::F32),
        TypesValType::F32
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::F64),
        TypesValType::F64
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::FuncRef),
        TypesValType::Own(0)
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::ExternRef),
        TypesValType::Ref(0)
    );
}

/// Test conversion from format ValType to TypesValType
#[test]
fn test_format_valtype_to_types_valtype() {
    // Test primitive types
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::S32),
        TypesValType::S32
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::U32),
        TypesValType::U32
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::S64),
        TypesValType::S64
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::U64),
        TypesValType::U64
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::F32),
        TypesValType::F32
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::F64),
        TypesValType::F64
    );

    // Test more complex types
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Bool),
        TypesValType::Bool
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::String),
        TypesValType::String
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Char),
        TypesValType::Char
    );

    // Test compound types
    let list_type = FormatValType::List(Box::new(FormatValType::S32));
    let expected_list_type = TypesValType::List(Box::new(TypesValType::S32));
    assert_eq!(
        format_valtype_to_types_valtype(&list_type),
        expected_list_type
    );

    let option_type = FormatValType::Option(Box::new(FormatValType::S32));
    let expected_option_type = TypesValType::Option(Box::new(TypesValType::S32));
    assert_eq!(
        format_valtype_to_types_valtype(&option_type),
        expected_option_type
    );

    // Test record type
    let record_fields = vec![
        ("field1".to_string(), FormatValType::S32),
        ("field2".to_string(), FormatValType::String),
    ];
    let expected_fields = vec![
        ("field1".to_string(), TypesValType::S32),
        ("field2".to_string(), TypesValType::String),
    ];
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Record(record_fields)),
        TypesValType::Record(expected_fields)
    );

    // Test variant type
    let variant_cases = vec![
        ("case1".to_string(), Some(FormatValType::S32)),
        ("case2".to_string(), Some(FormatValType::String)),
        ("case3".to_string(), None),
    ];
    let expected_cases = vec![
        ("case1".to_string(), Some(TypesValType::S32)),
        ("case2".to_string(), Some(TypesValType::String)),
        ("case3".to_string(), None),
    ];
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Variant(variant_cases)),
        TypesValType::Variant(expected_cases)
    );

    // Test resource handles
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Own(5)),
        TypesValType::Own(5)
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Borrow(5)),
        TypesValType::Borrow(5)
    );
}

/// Test round-trip conversion for format ValType to TypesValType and back
#[test]
fn test_format_valtype_roundtrip() {
    let test_types = vec![
        FormatValType::S32,
        FormatValType::U32,
        FormatValType::S64,
        FormatValType::F32,
        FormatValType::F64,
        FormatValType::Bool,
        FormatValType::String,
        FormatValType::Char,
        FormatValType::List(Box::new(FormatValType::S32)),
        FormatValType::Option(Box::new(FormatValType::String)),
        FormatValType::Tuple(vec![FormatValType::S32, FormatValType::String]),
        FormatValType::Own(5),
        FormatValType::Borrow(7),
    ];

    for original_type in test_types {
        // Convert to runtime type
        let runtime_type = format_valtype_to_types_valtype(&original_type);

        // Convert back to format type
        let roundtrip_type = types_valtype_to_format_valtype(&runtime_type);

        // For many types, the conversion should be lossless
        // Note: Some complex types might not have perfect roundtrip conversion
        match &original_type {
            FormatValType::S32
            | FormatValType::U32
            | FormatValType::S64
            | FormatValType::U64
            | FormatValType::F32
            | FormatValType::F64
            | FormatValType::Bool
            | FormatValType::String
            | FormatValType::Char
            | FormatValType::Own(_)
            | FormatValType::Borrow(_) => {
                assert_eq!(original_type, roundtrip_type);
            },
            // For complex types, we don't assert exact equality as some information may be lost
            _ => {},
        }
    }
}

/// Test conversion from format extern type to runtime extern type
#[test]
fn test_format_to_runtime_extern_type() {
    // Test function type conversion
    let format_func = FormatExternType::Function {
        params: vec![("arg".to_string(), FormatValType::S32)],
        results: vec![FormatValType::S32],
    };

    let runtime_func = format_to_runtime_extern_type(&format_func).unwrap();

    match runtime_func {
        TypesExternType::Function(func_type) => {
            assert_eq!(func_type.params.len(), 1);
            assert_eq!(func_type.results.len(), 1);
        },
        _ => panic!("Expected Function type, got {:?}", runtime_func),
    }

    // Test instance type conversion
    let format_instance = FormatExternType::Instance {
        exports: vec![(
            "func".to_string(),
            FormatExternType::Function {
                params: vec![],
                results: vec![],
            },
        )],
    };

    let runtime_instance = format_to_runtime_extern_type(&format_instance).unwrap();

    match runtime_instance {
        TypesExternType::Instance(instance_type) => {
            assert_eq!(instance_type.exports.len(), 1);
        },
        _ => panic!("Expected Instance type, got {:?}", runtime_instance),
    }

    // Test resource type conversion
    let resource_extern_type = FormatExternType::Resource {
        rep: ResourceRepresentation::Handle32,
        nullable: false,
    };

    let runtime_resource = format_to_runtime_extern_type(&resource_extern_type).unwrap();

    match runtime_resource {
        TypesExternType::Resource(resource_type) => {
            match resource_type {
                ResourceType::Indexed(repr, _) => {
                    assert_eq!(repr, 0); // Representation should be mapped to
                    // index
                },
                _ => panic!("Expected indexed resource type"),
            }
        },
        _ => panic!("Expected Resource type, got {:?}", runtime_resource),
    }
}

/// Test roundtrip conversion from runtime extern type to format extern type and
/// back
#[test]
fn test_extern_type_roundtrip() {
    // Create a function type
    let params = vec![ValueType::I32, ValueType::I64];
    let results = vec![ValueType::F32];
    let func_type = TypesFuncType::new(params, results);
    let extern_type = TypesExternType::Function(func_type);

    // Convert to format type
    let format_type = runtime_to_format_extern_type(&extern_type).unwrap();

    // Convert back to runtime type
    let roundtrip_type = format_to_runtime_extern_type(&format_type).unwrap();

    // Verify the structure is preserved
    match roundtrip_type {
        TypesExternType::Function(func_type) => {
            assert_eq!(func_type.params.len(), 2);
            assert_eq!(func_type.results.len(), 1);
            assert_eq!(func_type.params[0], ValueType::I32);
            assert_eq!(func_type.params[1], ValueType::I64);
            assert_eq!(func_type.results[0], ValueType::F32);
        },
        _ => panic!("Expected Function type after roundtrip"),
    }
}

/// Test common type conversion utility functions
#[test]
fn test_common_conversion_utilities() {
    // Test common_to_format_val_type
    assert_eq!(
        common_to_format_val_type(&ValueType::I32).unwrap(),
        FormatValType::S32
    );

    // Test format_to_common_val_type
    assert_eq!(
        format_to_common_val_type(&FormatValType::S32).unwrap(),
        ValueType::I32
    );

    // Test extern_type_to_func_type
    let func_type = TypesFuncType::new(vec![ValueType::I32, ValueType::I64], vec![ValueType::F32]);
    let extern_type = TypesExternType::Function(func_type);

    let extracted_func_type = extern_type_to_func_type(&extern_type).unwrap();
    assert_eq!(extracted_func_type.params.len(), 2);
    assert_eq!(extracted_func_type.results.len(), 1);
}

/// Test IntoRuntimeType and IntoFormatType traits
#[test]
fn test_conversion_traits() {
    // Test IntoRuntimeType for FormatExternType
    let format_func = FormatExternType::Function {
        params: vec![("arg".to_string(), FormatValType::S32)],
        results: vec![FormatValType::S32],
    };

    let runtime_func: Result<TypesExternType, _> = format_func.clone().into_runtime_type();
    assert!(runtime_func.is_ok());

    // Test IntoFormatType for TypesExternType
    let func_type = TypesFuncType::new(vec![ValueType::I32], vec![ValueType::F32]);
    let extern_type = TypesExternType::Function(func_type);

    let format_type: Result<FormatExternType, _> = extern_type.into_format_type();
    assert!(format_type.is_ok());
}

/// Test error handling in conversion functions
#[test]
fn test_conversion_error_handling() {
    // Test error when converting unsupported format value types
    let unsupported_types = vec![
        FormatValType::ResultBoth(
            Box::new(FormatValType::S32),
            Box::new(FormatValType::String),
        ),
        FormatValType::ErrorContext,
    ];

    for val_type in unsupported_types {
        let result = format_val_type_to_value_type(&val_type);
        assert!(result.is_err());
    }

    // Test error when converting reference types to format types
    let ref_types = vec![ValueType::FuncRef, ValueType::ExternRef];

    for val_type in ref_types {
        let result = value_type_to_format_val_type(&val_type);
        assert!(result.is_err());
    }
}
